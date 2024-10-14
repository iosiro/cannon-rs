use alloy_json_abi::JsonAbi;
use alloy_primitives::{hex::ToHexExt, Address, B256};
use eyre::Result;
use foundry_compilers::{Project, ProjectCompileOutput};

use crate::generated::routers::utils::to_constant_case;

use super::{render_modules_with_template, Module};

pub fn generate_router(
    project: &Project,
    output: &ProjectCompileOutput,
    router_name: String,
    module_names: Vec<String>,
    deployer: Address,
    salt: B256,
) -> Result<String> {
    super::generate_router(
        project,
        output,
        router_name,
        module_names,
        Some(deployer),
        Some(salt),
        &|m: &Module| {
            format!(
                "case {} {{ result := {} }} // {}.{}()",
                m.selector.encode_hex_with_prefix(),
                to_constant_case(&m.contract_name),
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
            "    address constant {} = {};",
            to_constant_case(&m.contract_name),
            &m.address.unwrap().to_checksum(None)
        )
    });

    // Create the router file content.
    let router_content = include_str!("../../../assets/templates/RouterTemplate.sol");
    let router_content = router_content
        .replace("{interface}", &interface)
        .replace("{router_name}", &router_name)
        .replace("{modules}", &module_lookup);

    Ok(router_content)
}
