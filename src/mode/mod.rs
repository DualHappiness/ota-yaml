use anyhow::Result;
use serde::{Deserialize, Serialize};

mod components;
mod edit;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub enum Mode {
    Edit,
    Components,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Edit => write!(f, "edit"),
            Mode::Components => write!(f, "components"),
        }
    }
}

pub trait YamlHandle {
    fn handle(
        &mut self,
        ota: &super::Ota,
        vehicle: &super::Vehicle,
        yaml: &serde_yaml::Value,
    ) -> Result<serde_yaml::Value>;
}

pub fn get_handle_mode() -> Result<Option<Mode>> {
    let mode = inquire::Select::new("process mode", vec![Mode::Edit, Mode::Components])
        .prompt_skippable()?;
    Ok(mode)
}

pub fn get_handle(mode: &Mode) -> Box<dyn YamlHandle> {
    match mode {
        Mode::Edit => Box::new(edit::Data::new()) as Box<dyn YamlHandle>,
        Mode::Components => Box::new(components::Data::new()) as Box<dyn YamlHandle>,
    }
}
