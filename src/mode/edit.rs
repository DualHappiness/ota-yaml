pub struct Data {}

impl super::YamlHandle for Data {
    fn handle(
        &self,
        _ota: &crate::Ota,
        vehicle: &crate::Vehicle,
        yaml: &serde_yaml::Value,
    ) -> anyhow::Result<serde_yaml::Value> {
        let edited = inquire::Editor::new(&format!("edit {} yaml", vehicle.name))
            .with_editor_command(&std::ffi::OsStr::new("vim"))
            .with_predefined_text(&serde_yaml::to_string(yaml)?)
            .with_file_extension(".yaml")
            .prompt()?;
        Ok(serde_yaml::from_str(&edited)?)
    }
}
