use colored::*;
use std::collections::HashSet;

pub struct Data {
    selected: Option<HashSet<String>>,
}

impl Data {
    pub fn new() -> Self {
        Data { selected: None }
    }
}

impl super::YamlHandle for Data {
    fn handle(
        &mut self,
        _ota: &crate::Ota,
        _vehicle: &crate::Vehicle,
        yaml: &serde_yaml::Value,
    ) -> anyhow::Result<serde_yaml::Value> {
        let selected: HashSet<String>;
        let mut yaml = yaml.clone();
        if self.selected.is_some()
            && inquire::Confirm::new(&"[components] redo last".color("yellow"))
                .with_default(true)
                .prompt()?
        {
            selected = self.selected.as_ref().unwrap().clone();
        } else {
            let mut components = vec![];
            let mut enabled = vec![];
            for xxk in ["xxka", "xxkb"] {
                for (idx, comp) in yaml[xxk]["archon"]["component"]
                    .as_sequence()
                    .unwrap()
                    .iter()
                    .enumerate()
                {
                    components.push(comp["name"].as_str().unwrap().to_string());
                    if comp["enable"].as_bool().unwrap() {
                        enabled.push(idx);
                    }
                }
            }
            selected = inquire::MultiSelect::new("select enbale components", components)
                .with_default(&enabled)
                .prompt()?
                .into_iter()
                .collect();
        }
        for xxk in ["xxka", "xxkb"] {
            let components = yaml[xxk]["archon"]["component"].as_sequence_mut().unwrap();
            for component in components.iter_mut() {
                if !selected.contains(component["name"].as_str().unwrap()) {
                    component["enable"] = serde_yaml::Value::Bool(false);
                } else {
                    component["enable"] = serde_yaml::Value::Bool(true);
                }
            }
        }
        self.selected = Some(selected);
        Ok(yaml)
    }
}
