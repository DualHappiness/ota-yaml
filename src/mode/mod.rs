use anyhow::Result;
use serde::{Deserialize, Serialize};

mod components;
mod edit;
mod merge;
mod scenario;

#[derive(Debug, Serialize, Deserialize)]
enum Mode {
    Edit,
    Merge,
    Scenario,
    Components,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Edit => write!(f, "edit"),
            Mode::Merge => write!(f, "merge"),
            Mode::Scenario => write!(f, "scenario"),
            Mode::Components => write!(f, "components"),
        }
    }
}

pub trait YamlHandle {
    fn handle(
        &self,
        ota: &super::Ota,
        vehicle: &super::Vehicle,
        yaml: &serde_yaml::Value,
    ) -> Result<serde_yaml::Value>;
}

pub fn get_handle() -> Result<Box<dyn YamlHandle>> {
    let mode = inquire::Select::new(
        "process mode",
        vec![Mode::Edit, Mode::Merge, Mode::Scenario, Mode::Components],
    )
    .prompt()?;
    let handle = match mode {
        Mode::Edit => Box::new(edit::Data {}) as Box<dyn YamlHandle>,
        Mode::Merge => Box::new(merge::Data {}) as Box<dyn YamlHandle>,
        Mode::Scenario => Box::new(scenario::Data {}) as Box<dyn YamlHandle>,
        Mode::Components => Box::new(components::Data {}) as Box<dyn YamlHandle>,
    };
    Ok(handle)
}
