use std::collections::HashSet;

pub struct Data {}

enum Scenario {
    Driver,
}

impl std::fmt::Display for Scenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scenario::Driver => write!(f, "driver"),
        }
    }
}

impl super::YamlHandle for Data {
    fn handle(
        &self,
        _ota: &crate::Ota,
        _vehicle: &crate::Vehicle,
        yaml: &serde_yaml::Value,
    ) -> anyhow::Result<serde_yaml::Value> {
        tracing::info!("scenario use json config to control components");
        let selected = inquire::Select::new("scenario", vec![Scenario::Driver]).prompt()?;
        let path = format!("./data/{}.json", selected);
        let components: Vec<String> = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let component_set: HashSet<_> = components.iter().map(|c| c.to_string()).collect();
        let mut yaml = yaml.clone();
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
