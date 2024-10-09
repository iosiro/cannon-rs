use alloy_json_abi::{Function, JsonAbi};
use alloy_primitives::{hex::ToHexExt, keccak256, Selector};
use clap::Parser;
use eyre::Result;
use foundry_cli::{opts::CoreBuildArgs, utils::LoadConfig};
use foundry_compilers::{
    artifacts::solc::ConfigurableContractArtifact,
    info::ContractInfo,
};
use foundry_config::{
    figment::{
        value::{Dict, Map},
        Metadata, Profile, Provider,
    },
    Config,
};
use itertools::Itertools;
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};
use yansi::Paint;

// Loads project's figment and merges the build cli arguments into it
foundry_config::merge_impl_figment_convert!(GenerateDynamicRouterArgs, opts);

/// CLI arguments for `forge generate router`.
#[derive(Clone, Debug, Default, Serialize, Parser)]
pub struct GenerateDynamicRouterArgs {
    /// Router name for router generation.
    #[clap(long, value_name = "ROUTER_NAME")]
    name: String,

    /// Contract names for router generation.
    module_names: Vec<String>,

    #[command(flatten)]
    opts: CoreBuildArgs,
}

impl GenerateDynamicRouterArgs {
    pub fn run(self) -> Result<()> {
        // Merge all configs.
        let config = self.try_load_config_emit_warnings()?;
        let project = config.create_project(true, true)?;

        let output = project.compile()?;

        if output.has_compiler_errors() {
            println!("{output}");
            eyre::bail!("Compilation failed");
        }

        let mut targets: HashMap<String, Option<(ContractInfo, ConfigurableContractArtifact)>> =
            HashMap::new();
        for target in &self.module_names {
            targets.insert(target.clone(), None);
        }

        for (path, name, info) in output.into_artifacts_with_files() {
            for (module_name, module_info) in &mut targets {
                let target = ContractInfo::new(module_name.as_str());
                if let Some(target_path) = &target.path {
                    if PathBuf::from(target_path) == path && target.name == name {
                        *module_info = Some((target, info.clone()));
                    } else if let Ok(resolved_path) = project
                        .paths
                        .resolve_import(project.root(), Path::new(&target_path))
                    {
                        if resolved_path == path && target.name == name {
                            *module_info = Some((target, info.clone()));
                        }
                    }
                }
            }
        }

        // Make sure all targets were found
        for (module_name, module_info) in &targets {
            if module_info.is_none() {
                eyre::bail!("Module `{}` not found", module_name);
            }
        }

        let sources = targets
            .into_iter()
            .filter_map(|(_, info)| info.map(|(info, artifact)| (info.name, artifact)))
            .collect::<Vec<(String, ConfigurableContractArtifact)>>();

        let output_dir = project
            .sources_path()
            .as_path()
            .to_path_buf()
            .join("generated/routers");

        let output = build_router(self.name.clone(), sources)?;

        let output_dir = Path::new(&output_dir);
        fs::create_dir_all(output_dir)?;

        let router_file_path = output_dir.join(format!("{}.g.sol", self.name));
        fs::write(&router_file_path, output)?;
        println!(
            "{} router file: {}",
            Paint::green("Generated"),
            router_file_path.to_str().unwrap()
        );

        Ok(())
    }
}

impl Provider for GenerateDynamicRouterArgs {
    fn metadata(&self) -> Metadata {
        Metadata::named("Generator Args Provider")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, foundry_config::figment::Error> {
        Ok(Map::from([(Config::selected_profile(), Dict::default())]))
    }
}

#[derive(Debug, Clone)]
struct RouterTemplateInputs {
    contract_name: String,
    function_name: String,
    selector: Selector,
}

#[derive(Debug, Clone)]
struct BinaryData {
    selectors: Vec<RouterTemplateInputs>,
    children: Vec<BinaryData>,
}

fn build_router(
    router_name: String,
    sources: Vec<(String, ConfigurableContractArtifact)>,
) -> Result<String> {
    let mut combined_abi = JsonAbi::new();
    let mut functions = BTreeMap::<Selector, Function>::new();
    let mut selectors = Vec::new();

    for module in sources.iter() {
        let (module_name, artifact) = module;

        let abi = artifact
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

                selectors.push(RouterTemplateInputs {
                    contract_name: module_name.clone(),
                    function_name: function.name.clone(),
                    selector: function.selector(),
                });
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

    let interface = combined_abi.to_sol(format!("I{router_name}").as_str(), None);

    let router_tree = build_binary_data(selectors.clone());
    let module_lookup = render_modules(selectors.clone());
    let resolver = render_resolver(selectors.clone());
    let constructor_args = render_constructor_args(selectors.clone());
    let immutables = render_immutables(selectors.clone());
    let struct_str = render_struct(selectors.clone());
    //let functions = render_interface(selectors.clone());

    let selectors = render_selectors(router_tree);

    // Create the router file content.
    let router_content = include_str!("../../../assets/templates/DynamicRouterTemplate.sol");
    let router_content = router_content
        .replace("{selectors}", &selectors)
        .replace("{resolver}", &resolver)
        .replace("{interface}", &interface)
        .replace("{router_name}", &router_name)
        .replace("{constructor_args}", &constructor_args)
        .replace("{immutables}", &immutables)
        .replace("{struct}", &struct_str)
        .replace("{modules}", &module_lookup);

    // Create the router directory if it doesn't exist.

    Ok(router_content)
}

fn build_binary_data(selectors: Vec<RouterTemplateInputs>) -> BinaryData {
    const MAX_SELECTORS_PER_SWITCH_STATEMENT: usize = 9;

    let mut selectors = selectors;
    selectors.sort_by(|a, b| a.selector.cmp(&b.selector));

    fn binary_split(node: &mut BinaryData) {
        if node.selectors.len() > MAX_SELECTORS_PER_SWITCH_STATEMENT {
            let mid_idx = (node.selectors.len() + 1) / 2;

            let mut child_a = BinaryData {
                selectors: node.selectors.drain(..mid_idx).collect(),
                children: Vec::new(),
            };

            let mut child_b = BinaryData {
                selectors: node.selectors.drain(..).collect(),
                children: Vec::new(),
            };

            binary_split(&mut child_a);
            binary_split(&mut child_b);

            node.children.push(child_a);
            node.children.push(child_b);
        }
    }

    let mut root = BinaryData {
        selectors,
        children: Vec::new(),
    };

    binary_split(&mut root);

    root
}

fn repeat_string(s: &str, count: usize) -> String {
    (0..count).map(|_| s).collect()
}

fn render_selectors(mut binary_data: BinaryData) -> String {
    let mut selectors_str: Vec<String> = Vec::new();

    fn render_node(node: &mut BinaryData, indent: usize, selectors_str: &mut Vec<String>) {
        if !node.children.is_empty() {
            let mut child_a = node.children.remove(0);
            let mut child_b = node.children.remove(0);

            fn find_mid_selector(node: &mut BinaryData) -> &RouterTemplateInputs {
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
                mid_selector.selector.encode_hex_with_prefix()
            ));
            render_node(&mut child_a, indent + 1, selectors_str);
            selectors_str.push(format!("{}}}", repeat_string("    ", indent)));

            render_node(&mut child_b, indent, selectors_str);
        } else {
            selectors_str.push(format!("{}switch sig", repeat_string("    ", indent)));
            for s in &node.selectors {
                selectors_str.push(format!(
                    "{}case {} {{ result := {} }} // {}.{}()",
                    repeat_string("    ", indent + 1),
                    s.selector.encode_hex_with_prefix(),
                    keccak256(to_constant_case(&s.contract_name)),
                    //to_constant_case(&s.contract_name),
                    s.contract_name,
                    s.function_name
                ));
            }
            selectors_str.push(format!("{}leave", repeat_string("    ", indent)));
        }
    }

    render_node(&mut binary_data, 4, &mut selectors_str);

    selectors_str.join("\n")
}

fn render_modules(modules: Vec<RouterTemplateInputs>) -> String {
    modules
        .iter()
        .map(|m| {
            format!(
                "    address immutable internal {};",
                to_constant_case(&m.contract_name)
            )
        })
        .unique()
        .collect::<Vec<String>>()
        .join("\n")
}

fn render_resolver(modules: Vec<RouterTemplateInputs>) -> String {
    modules
        .iter()
        .map(|m| {
            format!(
                "        if (implementation == {}) return {};",
                keccak256(to_constant_case(&m.contract_name)),
                to_constant_case(&m.contract_name)
            )
        })
        .unique()
        .collect::<Vec<String>>()
        .join("\n")
}

fn render_constructor_args(modules: Vec<RouterTemplateInputs>) -> String {
    modules
        .iter()
        .map(|m| format!("address {}", to_lower_camel_case(&m.contract_name)))
        .unique()
        .collect::<Vec<String>>()
        .join(", ")
}

fn render_immutables(modules: Vec<RouterTemplateInputs>) -> String {
    modules
        .iter()
        .map(|m| {
            format!(
                "        {} = $.{};",
                to_constant_case(&m.contract_name),
                to_lower_camel_case(&m.contract_name)
            )
        })
        .unique()
        .collect::<Vec<String>>()
        .join("\n")
}

fn render_struct(modules: Vec<RouterTemplateInputs>) -> String {
    modules
        .iter()
        .map(|m| format!("        address {};", to_lower_camel_case(&m.contract_name)))
        .unique()
        .collect::<Vec<String>>()
        .join("\n")
}

/// Utility function to convert an identifier to constant case.
fn to_constant_case(name: &str) -> String {
    let mut result = String::new();
    let mut prev_is_uppercase = false;

    for c in name.chars() {
        if c.is_uppercase() {
            if !prev_is_uppercase {
                result.push('_');
            }
            prev_is_uppercase = true;
        } else {
            prev_is_uppercase = false;
        }

        result.push(c);
    }

    result.to_uppercase()
}

fn to_lower_camel_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_lowercase().collect::<String>() + chars.as_str(),
    }
}
