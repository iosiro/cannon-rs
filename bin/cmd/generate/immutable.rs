use cannon_rs::generated::routers::{immutable::render_immutable_router, modules::{generate_router, identify_sources}, toml::TomlDefintion};
use clap::Parser;
use foundry_cli::{opts::CoreBuildArgs, utils::LoadConfig};
use foundry_config::{figment::{value::{Dict, Map}, Metadata, Profile, Provider}, Config};
use eyre::Result;

// Loads project's figment and merges the build cli arguments into it
foundry_config::merge_impl_figment_convert!(GenerateImmutableRouterArgs, opts);

/// CLI arguments for `forge generate router`.
#[derive(Clone, Debug, Default, Parser)]
pub struct GenerateImmutableRouterArgs {
    /// Generate from TOML configuration file.
    #[clap(long, value_name = "ROUTER_TOML")]
    toml: Option<String>,

    /// Router name for router generation.
    #[clap(long, value_name = "ROUTER_NAME", conflicts_with = "toml")]
    name: Option<String>,

    /// Contract names for router generation.
    #[clap(conflicts_with = "toml")]
    module_names: Vec<String>,

    #[command(flatten)]
    opts: CoreBuildArgs,
}

impl GenerateImmutableRouterArgs {
    pub fn run(&self) -> Result<()> {
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
            self.name.clone().unwrap(),
            self.module_names.clone(),
            render_immutable_router,
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
            generate_router(
                &project,
                &output,
                router_name.to_string(),
                router.modules.clone(),
                render_immutable_router,
            )?;
        }

        Ok(())
    }
}

impl Provider for GenerateImmutableRouterArgs {
    fn metadata(&self) -> Metadata {
        Metadata::named("Toml Generator Args Provider")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, foundry_config::figment::Error> {
        Ok(Map::from([(Config::selected_profile(), Dict::default())]))
    }
}