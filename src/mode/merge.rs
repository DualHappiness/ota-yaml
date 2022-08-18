pub struct Data {}

impl Data {
    fn variant_eq(a: &serde_yaml::Value, b: &serde_yaml::Value) -> bool {
        tracing::trace!(
            "variant_eq: {:?} {:?}",
            std::mem::discriminant(a),
            std::mem::discriminant(b),
        );
        std::mem::discriminant(a) == std::mem::discriminant(b)
    }

    fn merge_sequnce(
        old: &serde_yaml::Sequence,
        new: &serde_yaml::Sequence,
    ) -> serde_yaml::Sequence {
        if old.len() == 0 {
            return new.clone();
        }
        if new.len() == 0 {
            return old.clone();
        }
        if !old[0].is_mapping() || !old[0].as_mapping().unwrap().contains_key("name") {
            return new.clone();
        }
        let mut merged = old.clone();
        for item in merged.iter_mut() {
            if let Some(v) = new
                .iter()
                .filter_map(|i| i.as_mapping())
                .find(|i| i["name"] == item["name"])
            {
                *item = Data::merge_yaml(item, &serde_yaml::Value::Mapping(v.clone()))
            }
        }
        merged
    }

    fn merge_yaml(old: &serde_yaml::Value, new: &serde_yaml::Value) -> serde_yaml::Value {
        if !Data::variant_eq(old, new) {
            new.clone()
        } else if new.is_mapping() {
            let mut map = old.as_mapping().unwrap().clone();
            for (key, value) in new.as_mapping().unwrap().iter() {
                if map.contains_key(key) {
                    map[key] = Data::merge_yaml(&map[key], value);
                } else {
                    map.insert(key.clone(), value.clone());
                }
            }
            serde_yaml::Value::Mapping(map)
        } else if new.is_sequence() {
            let seq = Data::merge_sequnce(old.as_sequence().unwrap(), new.as_sequence().unwrap());
            serde_yaml::Value::Sequence(seq)
        } else {
            new.clone()
        }
    }
}

impl super::YamlHandle for Data {
    fn handle(
        &self,
        _ota: &crate::Ota,
        vehicle: &crate::Vehicle,
        yaml: &serde_yaml::Value,
    ) -> anyhow::Result<serde_yaml::Value> {
        let path = inquire::Text::new("content path")
            .with_default("./data/merge-template.yaml")
            .with_help_message("will use file content merge with yaml")
            .prompt()?;
        let content = std::fs::read_to_string(path).unwrap_or("".to_owned());
        let edited = inquire::Editor::new(&format!("edit {} yaml", vehicle.name))
            .with_predefined_text(&content)
            .with_editor_command(&std::ffi::OsStr::new("vim"))
            .with_file_extension(".yaml")
            .prompt()?;
        let new = serde_yaml::from_str(&edited)?;
        let merged = Data::merge_yaml(yaml, &new);
        tracing::debug!("merged: {:?}", merged);
        Ok(merged)
    }
}
