use std::collections::HashMap;

use alloy_json_abi::JsonAbi;
use alloy_primitives::{hex::ToHexExt, Selector};
use eyre::Result;
use itertools::Itertools;

use crate::generated::routers::utils::to_constant_case;

use super::{
    modules::Module,
    render::render_tree,
    tree::TreeNode,
    utils::to_lower_camel_case,
};


pub fn render_immutable_router(
    router_name: &String,
    root: &TreeNode,
    selectors: &HashMap<Selector, Module>,
    modules: &Vec<Module>,
    abi: &JsonAbi,
) -> Result<String> {
    let interface = abi.to_sol(format!("I{router_name}").as_str(), None);

    let tree = render_tree(root, selectors, render_dynamic_result);
    let module_lookup = render_modules(&modules);
    let resolver = render_resolver(&modules);
    let constructor_args = render_constructor_args(&modules);
    let immutables = render_immutables(&modules);
    let struct_str = render_struct(&modules);

    // Create the router file content.
    let router_content = include_str!("../../../assets/templates/ImmutableRouterTemplate.sol");
    let router_content = router_content
        .replace("{selectors}", &tree)
        .replace("{resolver}", &resolver)
        .replace("{interface}", &interface)
        .replace("{router_name}", &router_name)
        .replace("{constructor_args}", &constructor_args)
        .replace("{immutables}", &immutables)
        .replace("{struct}", &struct_str)
        .replace("{modules}", &module_lookup);

    Ok(router_content)
}

fn render_dynamic_result(input: &Module) -> String {
    format!(
        "case {} {{ result := {} }} // {}.{}()",
        input.selector.encode_hex_with_prefix(),
        input.contract_identifier,
        input.contract_name,
        input.function_name
    )
}

fn render_modules(modules: &Vec<Module>) -> String {
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

fn render_resolver(modules: &Vec<Module>) -> String {
    modules
        .iter()
        .map(|m| {
            format!(
                "        if (implementation == {}) return {};",
                m.contract_identifier,
                to_constant_case(&m.contract_name)
            )
        })
        .unique()
        .collect::<Vec<String>>()
        .join("\n")
}

fn render_constructor_args(modules: &Vec<Module>) -> String {
    modules
        .iter()
        .map(|m| format!("address {}", to_lower_camel_case(&m.contract_name)))
        .unique()
        .collect::<Vec<String>>()
        .join(", ")
}

fn render_immutables(modules: &Vec<Module>) -> String {
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

fn render_struct(modules: &Vec<Module>) -> String {
    modules
        .iter()
        .map(|m| format!("        address {};", to_lower_camel_case(&m.contract_name)))
        .unique()
        .collect::<Vec<String>>()
        .join("\n")
}
