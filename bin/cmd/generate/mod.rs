use std::{fs, path::Path};

use clap::{Parser, Subcommand};
use deterministic::GenerateRouterArgs;
use eyre::Result;
use foundry_compilers::Project;
use immutable::GenerateImmutableRouterArgs;
mod deterministic;
mod immutable;

/// CLI arguments for `forge generate`.
#[derive(Debug, Parser)]
pub struct GenerateArgs {
    #[command(subcommand)]
    pub sub: GenerateSubcommands,
}

#[derive(Debug, Subcommand)]
pub enum GenerateSubcommands {
    /// Generate ERCXXX router.
    Router(Box<GenerateRouterArgs>),
    ImmutableRouter(Box<GenerateImmutableRouterArgs>),
}

pub fn write_router(project: &Project, router: &str, router_name: &str) -> Result<String> {
    let output_dir = project
        .sources_path()
        .as_path()
        .to_path_buf()
        .join("generated/routers");

    let output_dir = Path::new(&output_dir);
    fs::create_dir_all(output_dir)?;

    let router_file_path = output_dir.join(format!("{}.g.sol", router_name));
    fs::write(&router_file_path, router)?;

    Ok(router_file_path.as_path().to_str().unwrap().to_string())
}
