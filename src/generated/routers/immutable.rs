use crate::generated::routers::utils::{to_constant_case, to_lower_camel_case};
use alloy_json_abi::JsonAbi;
use alloy_primitives::hex::ToHexExt;
use eyre::Result;
use foundry_compilers::{Project, ProjectCompileOutput};

use super::{render_modules_with_template, Module};

pub fn generate_router(
    project: &Project,
    output: &ProjectCompileOutput,
    router_name: String,
    module_names: Vec<String>,
) -> Result<String> {
    super::generate_router(
        project,
        output,
        router_name,
        module_names,
        None,
        None,
        &|m: &Module| {
            format!(
                "case {} {{ result := {} }} // {}.{}()",
                m.selector.encode_hex_with_prefix(),
                m.contract_identifier,
                m.contract_name,
                m.function_name
            )
        },
        &template,
    )
}

fn template(router_name: &String, modules: &Vec<Module>, abi: &JsonAbi) -> Result<String> {
    let interface = abi.to_sol(format!("I{router_name}").as_str(), None);

    let module_lookup = render_modules_with_template(modules, &|m| {
        format!(
            "    address immutable internal {};",
            to_constant_case(&m.contract_name)
        )
    });
    let resolver = render_modules_with_template(modules, &|m| {
        format!(
            "        if (implementation == {}) return {};",
            m.contract_identifier,
            to_constant_case(&m.contract_name)
        )
    });
    let constructor_args = render_modules_with_template(modules, &|m| {
        format!("address {}", to_lower_camel_case(&m.contract_name))
    });
    let immutables = render_modules_with_template(modules, &|m| {
        format!(
            "        {} = $.{};",
            to_constant_case(&m.contract_name),
            to_lower_camel_case(&m.contract_name)
        )
    });
    let struct_str = render_modules_with_template(modules, &|m| {
        format!("        address {};", to_lower_camel_case(&m.contract_name))
    });

    // Create the router file content.
    let router_content = include_str!("../../../assets/templates/ImmutableRouterTemplate.sol");
    let router_content = router_content
        .replace("{resolver}", &resolver)
        .replace("{interface}", &interface)
        .replace("{router_name}", &router_name)
        .replace("{constructor_args}", &constructor_args)
        .replace("{immutables}", &immutables)
        .replace("{struct}", &struct_str)
        .replace("{modules}", &module_lookup);

    Ok(router_content)
}
