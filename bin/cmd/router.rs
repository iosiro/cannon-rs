use alloy_primitives::{Address, Selector, B256};
use clap::{Parser, Subcommand};
use eyre::Result;
use foundry_cli::opts::{CompilerArgs, CoreBuildArgs, ProjectPathsArgs};
use foundry_common::{
    compile::{ProjectCompiler, SkipBuildFilter, SkipBuildFilters},
    fs,
};
use foundry_compilers::artifacts::output_selection::ContractOutputSelection;
use std::collections::BTreeMap;
use std::path::Path;
use yansi::Paint;

use alloy_json_abi::{Function, JsonAbi};
use foundry_compilers::{artifacts::CompactContractBytecode, info::ContractInfo, Project};
use hex::ToHexExt;
use itertools::Itertools;

/// CLI arguments for `cannon-rs router`.
#[derive(Debug, Parser)]
pub struct RouterArgs {
    #[command(subcommand)]
    pub sub: RouterSubcommands,
}

#[derive(Debug, Subcommand)]
pub enum RouterSubcommands {
    /// Scaffolds router file for given contracts
    Generate(Box<GenerateRouterArgs>),
}

#[derive(Debug, Parser)]
pub struct GenerateRouterArgs {
    /// Router name for router generation.
    #[clap(long, value_name = "ROUTER_NAME")]
    pub name: String,

    #[clap(long, default_value = "0x4e59b44847b379578588920ca78fbf26c0b4956c")]
    deployer: Address,

    #[clap(
        long,
        default_value = "0x0000000000000000000000000000000000000000000000000000000000000000"
    )]
    salt: B256,

    /// Contract names for router generation.
    pub module_names: Vec<String>,

    #[clap(flatten)]
    pub project_paths: ProjectPathsArgs,
}

impl GenerateRouterArgs {
    pub fn run(self) -> Result<()> {
        let GenerateRouterArgs {
            deployer,
            name: router_name,
            module_names,
            salt,
            project_paths,
        } = self;

        let build_args = CoreBuildArgs {
            project_paths: project_paths.clone(),
            compiler: CompilerArgs {
                extra_output: vec![ContractOutputSelection::Abi],
                ..Default::default()
            },
            ..Default::default()
        };

        let project = build_args.project()?;

        let output_dir = project
            .sources_path()
            .as_path()
            .to_path_buf()
            .join("generated/routers");

        let filter = SkipBuildFilters::new(
            [SkipBuildFilter::Custom(format!(
                "{}/**.sol",
                output_dir.to_str().unwrap()
            ))],
            project.root().clone(),
        )?;

        ProjectCompiler::new()
            .filter(Box::new(filter))
            .quiet(true)
            .compile(&project)?;

        let output = build_router(&project, router_name.clone(), module_names, deployer, salt)?;

        let output_dir = Path::new(&output_dir);
        fs::create_dir_all(output_dir)?;

        let router_file_path = output_dir.join(format!("{}.g.sol", router_name));
        fs::write(&router_file_path, output)?;
        println!(
            "{} router file: {}",
            Paint::green("Generated"),
            router_file_path.to_str().unwrap()
        );

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RouterTemplateInputs {
    address: Address,
    contract_name: String,
    function_name: String,
    selector: Selector,
}

#[derive(Debug, Clone)]
struct BinaryData {
    selectors: Vec<RouterTemplateInputs>,
    children: Vec<BinaryData>,
}

pub(crate) fn build_router(
    project: &Project,
    router_name: String,
    module_names: Vec<String>,
    deployer: Address,
    salt: B256,
) -> Result<String> {
    let router_name = format_identifier(&router_name, true);

    let cache = project.read_cache_file()?;
    let cached_artifacts = cache.read_artifacts::<CompactContractBytecode>()?;

    let mut combined_abi = JsonAbi::new();
    let mut functions = BTreeMap::<Selector, Function>::new();
    let mut selectors = Vec::new();

    for module_name in module_names.iter() {
        let ContractInfo {
            name: module_name,
            path: module_path,
        } = ContractInfo::new(module_name);

        let cached_artifact = match module_path {
            Some(path) => project
                .paths
                .resolve_import(project.root(), Path::new(&path))
                .ok()
                .and_then(|path| cached_artifacts.find(path, module_name.clone())),
            None => cached_artifacts.find_first(module_name.clone()),
        }
        .ok_or_else(|| eyre::eyre!("No cached artifact found for contract `{module_name}`"))?;

        let bytecode = cached_artifact
            .bytecode
            .as_ref()
            .and_then(|b| b.bytes())
            .ok_or_else(|| eyre::eyre!("No bytecode found for contract `{module_name}`"))?;

        // calculate create2 address
        let address = Address::create2_from_code(&deployer, salt, bytecode);

        let abi = cached_artifact
            .abi
            .as_ref()
            .ok_or_else(|| eyre::eyre!("No ABI found for contract `{module_name}`"))?;

        for function_set in abi.functions.iter() {
            for function in function_set.1.iter() {
                if functions.contains_key(&function.selector()) {
                    return Err(eyre::eyre!("Duplicate selector found"));
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
                    address,
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

    for (_, function) in functions.iter() {
        combined_abi
            .functions
            .insert(function.name.clone(), vec![function.clone()]);
    }

    let interface = combined_abi.to_sol(format!("I{}", router_name).as_str(), None);

    let router_tree = build_binary_data(selectors.clone());
    let module_lookup = render_modules(selectors.clone());
    //let functions = render_interface(selectors.clone());

    let selectors = render_selectors(router_tree);

    // Create the router file content.
    let router_content = include_str!("../../assets/templates/RouterTemplate.sol");
    let router_content = router_content
        .replace("{selectors}", &selectors)
        .replace("{interface}", &interface)
        .replace("{router_name}", &router_name)
        .replace("{modules}", &module_lookup);

    // Create the router directory if it doesn't exist.

    Ok(router_content)
}

fn build_binary_data(selectors: Vec<RouterTemplateInputs>) -> BinaryData {
    const MAX_SELECTORS_PER_SWITCH_STATEMENT: usize = 9;

    let mut selectors = selectors.clone();
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

            render_node(&mut child_b, indent + 2, selectors_str);
        } else {
            selectors_str.push(format!("{}switch sig", repeat_string("    ", indent)));
            for s in &node.selectors {
                selectors_str.push(format!(
                    "{}case {} {{ result := {} }} // {}.{}()",
                    repeat_string("    ", indent + 1),
                    s.selector.encode_hex_with_prefix(),
                    to_constant_case(&s.contract_name),
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
    let mut modules_str: Vec<String> = Vec::new();

    let modules = modules
        .clone()
        .into_iter()
        .unique_by(|m| m.contract_name.clone())
        .collect::<Vec<RouterTemplateInputs>>();

    for RouterTemplateInputs {
        address,
        contract_name,
        ..
    } in modules
    {
        modules_str.push(format!(
            "address constant {} = {};",
            to_constant_case(&contract_name),
            address.to_checksum(None)
        ));
    }

    modules_str.join("\n")
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

/// Utility function to convert an identifier to pascal or camel case.
fn format_identifier(input: &str, is_pascal_case: bool) -> String {
    let mut result = String::new();
    let mut capitalize_next = is_pascal_case;

    for word in input.split_whitespace() {
        if !word.is_empty() {
            let (first, rest) = word.split_at(1);
            let formatted_word = if capitalize_next {
                format!("{}{}", first.to_uppercase(), rest)
            } else {
                format!("{}{}", first.to_lowercase(), rest)
            };
            capitalize_next = true;
            result.push_str(&formatted_word);
        }
    }
    result
}
