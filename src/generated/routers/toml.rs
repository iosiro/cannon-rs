use eyre::Result;
use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Debug, Deserialize)]
pub struct Router {
    pub modules: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TomlDefintion {
    #[serde(rename = "router")]
    pub routers: HashMap<String, Router>,
}

impl TomlDefintion {
    pub fn from_path(path: PathBuf) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let toml: TomlDefintion = toml::from_str(&content)?;
        Ok(toml)
    }
}
