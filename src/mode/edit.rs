use std::hash::{Hash, Hasher};

use anyhow::Result;
use colored::*;
type Yaml = serde_yaml::Value;

#[derive(Debug, Clone)]
enum PathKey {
    Key(Yaml),
    Index(usize),
    NameIndex(String),
}

#[derive(Debug, Clone)]
enum Operation {
    Add(Vec<PathKey>, Yaml),
    Mod(Vec<PathKey>, Yaml),
    Del(Vec<PathKey>, Yaml),
}

pub struct Data {
    diff: Option<Vec<Operation>>,
}

impl Data {
    pub fn new() -> Self {
        Data { diff: None }
    }

    fn name(yaml: &Yaml) -> Option<&str> {
        if let Some(name) = yaml["name"].as_str() {
            return Some(name);
        }
        if let Some(name) = yaml["key"].as_str() {
            return Some(name);
        }
        None
    }

    fn hash(v: &Yaml) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        v.hash(&mut hasher);
        hasher.finish()
    }

    fn recurse_diff(
        old: &Yaml,
        new: &Yaml,
        path: &mut Vec<PathKey>,
        diff: &mut Vec<Operation>,
    ) -> Result<()> {
        if std::mem::discriminant(old) != std::mem::discriminant(new) {
            diff.push(Operation::Mod(path.clone(), new.clone()));
            return Ok(());
        }
        if Data::hash(old) == Data::hash(new) {
            return Ok(());
        }
        if new.is_mapping() {
            let old_map = old.as_mapping().unwrap();
            let new_map = new.as_mapping().unwrap();

            for (key, value) in new_map.iter() {
                if old_map.contains_key(key) {
                    path.push(PathKey::Key(key.clone()));
                    Data::recurse_diff(&old_map[key], value, path, diff)?;
                    path.pop();
                } else {
                    diff.push(Operation::Add(path.clone(), value.clone()));
                }
            }
            for (key, value) in old_map.iter() {
                if !new_map.contains_key(key) {
                    diff.push(Operation::Del(path.clone(), value.clone()));
                }
            }
        } else if new.is_sequence() {
            let old_seq = old.as_sequence().unwrap();
            let new_seq = new.as_sequence().unwrap();
            let has_name = old_seq.len() > 0 && Data::name(&old_seq[0]).is_some();
            let has_name = has_name || new_seq.len() > 0 && Data::name(&new_seq[0]).is_some();
            if has_name {
                for new_item in new_seq {
                    if let Some(new_name) = Data::name(new_item) {
                        path.push(PathKey::NameIndex(new_name.to_string()));
                        if let Some(old_idx) = old_seq
                            .iter()
                            .position(|i| Data::name(i).map(|n| n == new_name).unwrap_or(false))
                        {
                            Data::recurse_diff(&old_seq[old_idx], new_item, path, diff)?;
                        } else {
                            diff.push(Operation::Add(path.clone(), new_item.clone()));
                        }
                        path.pop();
                    } else {
                        tracing::error!("no name in new sequence: {:?}", new_item);
                    }
                }
                for old_item in old_seq {
                    if let Some(old_name) = Data::name(old_item) {
                        path.push(PathKey::NameIndex(old_name.to_string()));
                        if new_seq
                            .iter()
                            .position(|i| Data::name(i).map(|n| n == old_name).unwrap_or(false))
                            .is_none()
                        {
                            diff.push(Operation::Del(path.clone(), old_item.clone()));
                        }
                        path.pop();
                    } else {
                        tracing::error!("no name in old sequence: {:?}", old_item);
                    }
                }
            } else {
                for i in (0..std::cmp::max(old_seq.len(), new_seq.len())).rev() {
                    path.push(PathKey::Index(i));
                    if i > old_seq.len() {
                        diff.push(Operation::Add(path.clone(), new_seq[i].clone()));
                    } else if i > new_seq.len() {
                        diff.push(Operation::Del(path.clone(), old_seq[i].clone()));
                    } else {
                        Data::recurse_diff(&old_seq[i], &new_seq[i], path, diff)?;
                    }
                    path.pop();
                }
            }
        } else {
            diff.push(Operation::Mod(path.clone(), new.clone()));
        }
        Ok(())
    }

    fn diff(old: &Yaml, new: &Yaml) -> Result<Vec<Operation>> {
        let mut diff = vec![];
        let mut path = vec![];
        Data::recurse_diff(old, new, &mut path, &mut diff)?;
        Ok(diff)
    }

    fn seek<'a, 'b>(root: &'a mut Yaml, path: &'b [PathKey]) -> &'a mut Yaml {
        let mut cur = root;
        for p in path {
            cur = match p {
                PathKey::Key(key) => &mut cur[key],
                PathKey::Index(index) => &mut cur.as_sequence_mut().unwrap()[*index],
                PathKey::NameIndex(name) => {
                    let named = cur.as_sequence_mut().expect("Named sequence");
                    if let Some(idx) = named
                        .iter()
                        .position(|item| Data::name(item).map(|n| n == name).unwrap_or(false))
                    {
                        &mut named[idx]
                    } else {
                        named.push(Yaml::Mapping(serde_yaml::mapping::Mapping::new()));
                        named.last_mut().unwrap()
                    }
                }
            }
        }
        cur
    }

    fn apply(yaml: &Yaml, operations: &[Operation]) -> Result<Yaml> {
        let mut yaml = yaml.clone();

        for op in operations {
            match op {
                Operation::Add(path, value) | Operation::Mod(path, value) => {
                    let cur = Data::seek(&mut yaml, path.as_slice());
                    *cur = value.clone();
                }
                Operation::Del(path, _value) => {
                    let cur = Data::seek(&mut yaml, &path[..path.len() - 1]);
                    match path.last().unwrap() {
                        PathKey::Key(key) => {
                            cur.as_mapping_mut().unwrap().remove(key);
                        }
                        PathKey::Index(index) => {
                            cur.as_sequence_mut().unwrap().remove(*index);
                        }
                        PathKey::NameIndex(name) => {
                            let named = cur.as_sequence_mut().unwrap();
                            named.retain(|item| {
                                item["name"].as_str().unwrap() != name
                                    && item["key"].as_str().unwrap() != name
                            });
                        }
                    };
                }
            }
        }
        Ok(yaml)
    }
}

impl super::YamlHandle for Data {
    fn handle(
        &mut self,
        _ota: &crate::Ota,
        vehicle: &crate::Vehicle,
        yaml: &serde_yaml::Value,
    ) -> anyhow::Result<serde_yaml::Value> {
        let mut yaml = yaml.clone();
        if self.diff.is_some()
            && inquire::Confirm::new(&"[edit] redo last".color("yellow"))
                .with_default(true)
                .prompt()?
        {
            yaml = Data::apply(&yaml, self.diff.as_ref().unwrap())?;
        } else {
            let edited = inquire::Editor::new(&format!("edit {} yaml", vehicle.name))
                .with_editor_command(&std::ffi::OsStr::new("vim"))
                .with_predefined_text(&serde_yaml::to_string(&yaml)?)
                .with_file_extension(".yaml")
                .prompt()?;
            let edited_yaml = serde_yaml::from_str(&edited)?;
            self.diff = Some(Data::diff(&yaml, &edited_yaml)?);
            tracing::debug!("diff: {:?}", self.diff);
            yaml = edited_yaml;
        }
        Ok(yaml)
    }
}
