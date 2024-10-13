use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use alloy_json_abi::{Function, JsonAbi};
use alloy_primitives::hex::ToHexExt;
use alloy_primitives::{keccak256, Selector};
use eyre::{eyre, Result};
use foundry_compilers::artifacts::ConfigurableContractArtifact;
use foundry_compilers::info::ContractInfo;
use foundry_compilers::{Project, ProjectCompileOutput};
use itertools::Itertools;
use yansi::Paint;

use super::tree::{build_binary_tree, TreeNode};
use super::utils::to_constant_case;

#[derive(Debug, Clone)]
pub struct Module {
    pub contract_identifier: String,
    pub contract_name: String,
    pub function_name: String,
    pub selector: Selector,
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

pub fn write_router(project: &Project, router: &str, router_name: &str) -> Result<()> {
    let output_dir = project
        .sources_path()
        .as_path()
        .to_path_buf()
        .join("generated/routers");

    let output_dir = Path::new(&output_dir);
    fs::create_dir_all(output_dir)?;

    let router_file_path = output_dir.join(format!("{}.g.sol", router_name));
    fs::write(&router_file_path, router)?;
    println!(
        "{} router file: {}",
        Paint::green("Generated"),
        router_file_path.to_str().unwrap()
    );

    Ok(())
}

pub fn generate_router<F>(
    project: &Project,
    output: &ProjectCompileOutput,
    router_name: String,
    module_names: Vec<String>,
    render_template: F,
) -> Result<()>
where
    F: Fn(&String, &TreeNode, &HashMap<Selector, Module>, &Vec<Module>, &JsonAbi) -> Result<String>,
{
    let (selectors, abi) = collect_sources(&project, &output, module_names.clone())?;

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

    let router_content = render_template(&router_name, &data, &selectors, &modules, &abi)?;

    write_router(project, &router_content, &router_name)?;

    Ok(())
}
