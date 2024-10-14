use alloy_primitives::{Address, B256};
use cannon_rs::generated::routers::deterministric::generate_router;
use cannon_rs::generated::routers::identify_sources;
use cannon_rs::generated::routers::toml::TomlDefintion;
use clap::Parser;
use eyre::Result;
use foundry_cli::{opts::CoreBuildArgs, utils::LoadConfig};
use foundry_config::{
    figment::{
        value::{Dict, Map},
        Metadata, Profile, Provider,
    },
    Config,
};
use serde::Serialize;
use yansi::Paint;

use crate::cmd::generate::write_router;

// Loads project's figment and merges the build cli arguments into it
foundry_config::merge_impl_figment_convert!(GenerateRouterArgs, opts);

/// CLI arguments for `forge generate router`.
#[derive(Clone, Debug, Default, Serialize, Parser)]
pub struct GenerateRouterArgs {
    /// Generate from TOML configuration file.
    #[clap(long, value_name = "ROUTER_TOML", conflicts_with_all = ["name", "module_names"])]
    toml: Option<String>,

    /// Router name for router generation.
    #[clap(long, value_name = "ROUTER_NAME")]
    name: Option<String>,

    #[clap(long, default_value = "0x4e59b44847b379578588920ca78fbf26c0b4956c")]
    deployer: Address,

    #[clap(
        long,
        default_value = "0x0000000000000000000000000000000000000000000000000000000000000000"
    )]
    salt: B256,

    /// Contract names for router generation.
    module_names: Vec<String>,

    #[command(flatten)]
    opts: CoreBuildArgs,
}

impl GenerateRouterArgs {
    pub fn run(self) -> Result<()> {
        if self.toml.is_some() {
            self.run_toml()?;
            return Ok(());
        }

        // Merge all configs.
        let config = self.try_load_config_emit_warnings()?;

        let project = config.create_project(true, true)?;

        let sources = identify_sources(&project, self.module_names.clone())?;

        let output = project.compile_files(sources)?;

        if output.has_compiler_errors() {
            println!("{output}");
            eyre::bail!("Compilation failed");
        }

        generate_router(
            &project,
            &output,
            self.name.unwrap().clone(),
            self.module_names.clone(),
            self.deployer,
            self.salt,
        )?;

        Ok(())
    }

    fn run_toml(&self) -> Result<()> {
        let config = self.try_load_config_emit_warnings()?;

        let project = config.create_project(true, true)?;

        let toml = TomlDefintion::from_path(self.toml.clone().unwrap().into())?;

        let module_names: Vec<String> = toml
            .routers
            .iter()
            .flat_map(|(_, router)| router.modules.clone())
            .collect();

        let sources = identify_sources(&project, module_names.clone())?;

        let output = project.compile_files(sources)?;

        if output.has_compiler_errors() {
            println!("{output}");
            eyre::bail!("Compilation failed");
        }

        for (router_name, router) in toml.routers.iter() {
            let source = generate_router(
                &project,
                &output,
                router_name.to_string(),
                router.modules.clone(),
                self.deployer,
                self.salt,
            )?;

            let router_file_path = write_router(&project, &source, router_name)?;

            println!(
                "{} router file: {}",
                Paint::green("Generated"),
                router_file_path
            );
        }

        Ok(())
    }
}

impl Provider for GenerateRouterArgs {
    fn metadata(&self) -> Metadata {
        Metadata::named("Generator Args Provider")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, foundry_config::figment::Error> {
        Ok(Map::from([(Config::selected_profile(), Dict::default())]))
    }
}
