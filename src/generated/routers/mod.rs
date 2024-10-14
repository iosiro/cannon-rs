use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};

use alloy_json_abi::{Function, JsonAbi};
use alloy_primitives::{hex::ToHexExt, keccak256, Address, Selector, B256};
use eyre::{eyre, Result};
use foundry_compilers::{
    artifacts::ConfigurableContractArtifact, info::ContractInfo, Project, ProjectCompileOutput,
};
use itertools::Itertools;
use utils::{repeat_string, to_constant_case};

pub mod deterministric;
pub mod immutable;
pub mod toml;

mod utils;

#[derive(Debug, Clone)]
pub struct Module {
    pub contract_identifier: String,
    pub contract_name: String,
    pub function_name: String,
    pub selector: Selector,
    pub address: Option<Address>,
}

/// Identity the source files for the given module names without compiling.
pub fn identify_sources(project: &Project, module_names: Vec<String>) -> Result<Vec<PathBuf>> {
    let mut sources = vec![];
    for module in module_names {
        let identifer = ContractInfo::new(module.as_str());
        let path = project.paths.resolve_import(
            project.root(),
            Path::new(&identifer.path.unwrap_or_else(|| module.clone())),
        )?;
        sources.push(path);
    }
    Ok(sources)
}

/// Collect the sources for the given module names from the project compile output.
pub fn collect_sources(
    project: &Project,
    output: &ProjectCompileOutput,
    module_names: Vec<String>,
    deployer: Option<Address>,
    salt: Option<B256>,
) -> Result<(HashMap<Selector, Module>, JsonAbi)> {
    let mut targets: HashMap<String, Option<(ContractInfo, ConfigurableContractArtifact)>> =
        module_names.into_iter().map(|name| (name, None)).collect();

    let mut remaining_modules = targets.len();

    for (path, name, info) in output.clone().into_artifacts_with_files() {
        for (module_name, module_info) in &mut targets {
            if module_info.is_some() {
                continue; // Skip already matched modules
            }

            let target = ContractInfo::new(module_name);
            if let Some(target_path) = &target.path {
                if is_matching_path(project, &path, target_path) && target.name == name {
                    *module_info = Some((target, info.clone()));
                    remaining_modules -= 1;
                    break;
                }
            }
        }

        if remaining_modules == 0 {
            break; // Exit early if all modules are found
        }
    }

    if remaining_modules > 0 {
        let missing_modules: Vec<String> = targets
            .into_iter()
            .filter_map(|(name, info)| if info.is_none() { Some(name) } else { None })
            .collect();
        return Err(eyre!("Modules not found: {}", missing_modules.join(", ")));
    }

    let sources: Vec<(String, ConfigurableContractArtifact)> = targets
        .into_iter()
        .map(|(_, info)| info.unwrap())
        .map(|(info, artifact)| (info.name, artifact))
        .sorted_by(|(a, _), (b, _)| a.cmp(b))
        .collect();

    let mut combined_abi = JsonAbi::new();
    let mut functions = BTreeMap::<Selector, Function>::new();
    let mut selectors = HashMap::new();

    for module in sources.iter() {
        let (module_name, artifact) = module;

        let address = if deployer.is_some() && salt.is_some() {
            let bytecode = artifact
                .bytecode
                .as_ref()
                .and_then(|b| b.bytes())
                .ok_or_else(|| eyre::eyre!("No bytecode found for contract `{module_name}`"))?;

            Some(Address::create2_from_code(
                &deployer.unwrap_or_default(),
                &salt.unwrap_or_default(),
                bytecode,
            ))
        } else {
            None
        };

        let abi: &JsonAbi = artifact
            .abi
            .as_ref()
            .ok_or_else(|| eyre::eyre!("No ABI found for contract `{module_name}`"))?;

        for function_set in abi.functions.iter() {
            for function in function_set.1.iter() {
                if functions.contains_key(&function.selector()) {
                    return Err(eyre::eyre!(format!(
                        "Duplicate selector found {}",
                        function.signature()
                    )));
                }

                functions.insert(function.selector(), function.clone());

                if let Some(f) = combined_abi.functions.get_mut(&function.name) {
                    f.push(function.clone());
                } else {
                    combined_abi
                        .functions
                        .insert(function.name.clone(), vec![function.clone()]);
                };

                let identifier =
                    keccak256(to_constant_case(module_name.as_str())).encode_hex_with_prefix();
                selectors.insert(
                    function.selector(),
                    Module {
                        contract_identifier: identifier,
                        contract_name: module_name.clone(),
                        function_name: function.name.clone(),
                        selector: function.selector(),
                        address,
                    },
                );
            }
        }

        if abi.fallback.is_some() {
            if combined_abi.fallback.is_some() {
                return Err(eyre::eyre!("Multiple fallback functions found"));
            }
            combined_abi.fallback = abi.fallback;
        }
        if abi.receive.is_some() {
            if combined_abi.receive.is_some() {
                return Err(eyre::eyre!("Multiple receive functions found"));
            }
            combined_abi.receive = abi.receive;
        }
    }

    Ok((selectors, combined_abi))
}

fn is_matching_path(project: &Project, actual_path: &Path, target_path: &str) -> bool {
    PathBuf::from(target_path) == actual_path
        || project
            .paths
            .resolve_import(project.root(), Path::new(target_path))
            .map_or(false, |resolved_path| resolved_path == actual_path)
}

fn generate_router<F, K>(
    project: &Project,
    output: &ProjectCompileOutput,
    router_name: String,
    module_names: Vec<String>,
    deployer: Option<Address>,
    salt: Option<B256>,
    selector_template: K,
    router_template: F,
) -> Result<String>
where
    F: Fn(&String, &Vec<Module>, &JsonAbi) -> Result<String>,
    K: Fn(&Module) -> String,
{
    let (selectors, abi) =
        collect_sources(&project, &output, module_names.clone(), deployer, salt)?;

    let leafs = selectors
        .iter()
        .map(|(selector, _)| selector.clone())
        .collect::<Vec<Selector>>();

    let data = build_binary_tree(leafs.clone());

    let modules = selectors
        .iter()
        .unique_by(|(_, m)| m.contract_identifier.clone())
        .sorted_by(|(_, a), (_, b)| a.contract_name.cmp(&b.contract_name))
        .map(|(_, m)| m.clone())
        .collect::<Vec<Module>>();

    let router_content = render_router(
        &router_name,
        &data,
        &selectors,
        &modules,
        &abi,
        selector_template,
        router_template,
    )?;

    Ok(router_content)
}

fn render_router<F, K>(
    router_name: &String,
    root: &TreeNode,
    selectors: &HashMap<Selector, Module>,
    modules: &Vec<Module>,
    abi: &JsonAbi,
    render_selector: K,
    render_template: F,
) -> Result<String>
where
    F: Fn(&String, &Vec<Module>, &JsonAbi) -> Result<String>,
    K: Fn(&Module) -> String,
{
    let tree = render_tree(root, selectors, &render_selector);

    let router_content = render_template(&router_name, &modules, &abi)?;

    let router_content = router_content.replace("{selectors}", &tree);

    Ok(router_content)
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    selectors: Vec<Selector>,
    children: Vec<TreeNode>,
}

fn build_binary_tree(selectors: Vec<Selector>) -> TreeNode {
    const MAX_SELECTORS_PER_SWITCH_STATEMENT: usize = 9;

    let mut selectors = selectors;
    selectors.sort_by(|a, b| a.cmp(&b));

    fn binary_split(node: &mut TreeNode) {
        if node.selectors.len() > MAX_SELECTORS_PER_SWITCH_STATEMENT {
            let mid_idx = (node.selectors.len() + 1) / 2;

            let mut child_a = TreeNode {
                selectors: node.selectors.drain(..mid_idx).collect(),
                children: Vec::new(),
            };

            let mut child_b = TreeNode {
                selectors: node.selectors.drain(..).collect(),
                children: Vec::new(),
            };

            binary_split(&mut child_a);
            binary_split(&mut child_b);

            node.children.push(child_a);
            node.children.push(child_b);
        }
    }

    let mut root = TreeNode {
        selectors,
        children: Vec::new(),
    };

    binary_split(&mut root);

    root
}

fn render_tree<F>(
    root: &TreeNode,
    selectors: &HashMap<Selector, Module>,
    render_selector: F,
) -> String
where
    F: Fn(&Module) -> String,
{
    let mut tree = root.clone();
    let mut selectors_str: Vec<String> = Vec::new();

    fn render_node<F>(
        node: &mut TreeNode,
        indent: usize,
        selectors_str: &mut Vec<String>,
        selectors: &HashMap<Selector, Module>,
        render_selector: &F,
    ) where
        F: Fn(&Module) -> String,
    {
        if !node.children.is_empty() {
            let mut child_a = node.children.remove(0);
            let mut child_b = node.children.remove(0);

            fn find_mid_selector(node: &mut TreeNode) -> &Selector {
                if !node.selectors.is_empty() {
                    &node.selectors[0]
                } else {
                    find_mid_selector(&mut node.children[0])
                }
            }

            let mid_selector = find_mid_selector(&mut child_b);

            selectors_str.push(format!(
                "{}if lt(sig, {}) {{",
                repeat_string("    ", indent),
                mid_selector.encode_hex_with_prefix()
            ));
            render_node(
                &mut child_a,
                indent + 1,
                selectors_str,
                selectors,
                render_selector,
            );
            selectors_str.push(format!("{}}}", repeat_string("    ", indent)));

            render_node(
                &mut child_b,
                indent,
                selectors_str,
                selectors,
                render_selector,
            );
        } else {
            selectors_str.push(format!("{}switch sig", repeat_string("    ", indent)));
            for selector in &node.selectors {
                selectors_str.push(format!(
                    "{}{}",
                    repeat_string("    ", indent + 1),
                    render_selector(selectors.get(selector).unwrap())
                ));
            }
            selectors_str.push(format!("{}leave", repeat_string("    ", indent)));
        }
    }

    render_node(
        &mut tree,
        4,
        &mut selectors_str,
        selectors,
        &render_selector,
    );

    selectors_str.join("\n")
}

pub fn render_modules_with_template<F>(modules: &Vec<Module>, template: &F) -> String
where
    F: Fn(&Module) -> String,
{
    modules
        .iter()
        .map(|m| template(m))
        .unique()
        .collect::<Vec<String>>()
        .join("\n")
}
