use std::collections::HashSet;

pub struct Data {}

impl super::YamlHandle for Data {
    fn handle(
        &self,
        _ota: &crate::Ota,
        _vehicle: &crate::Vehicle,
        yaml: &serde_yaml::Value,
    ) -> anyhow::Result<serde_yaml::Value> {
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
        let mut yaml = yaml.clone();
        let selected = inquire::MultiSelect::new("select enbale components", components)
            .with_default(&enabled)
            .prompt()?;
        let component_set: HashSet<_> = selected.iter().map(|c| c.to_string()).collect();
        for xxk in ["xxka", "xxkb"] {
            let components = yaml[xxk]["archon"]["component"].as_sequence_mut().unwrap();
            for component in components.iter_mut() {
                if !component_set.contains(component["name"].as_str().unwrap()) {
                    component["enable"] = serde_yaml::Value::Bool(false);
                } else {
                    component["enable"] = serde_yaml::Value::Bool(true);
                }
            }
        }

        Ok(yaml)
    }
}
