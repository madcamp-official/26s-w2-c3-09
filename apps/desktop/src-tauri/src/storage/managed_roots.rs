use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManagedRoot {
    pub root_id: String,
    pub root: String,
    pub display_name: String,
}

#[derive(Debug, Default)]
pub struct ManagedRootStore {
    roots: Mutex<BTreeMap<String, ManagedRoot>>,
    storage_path: Mutex<Option<PathBuf>>,
}

impl ManagedRootStore {
    pub fn load_from_file(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref().to_path_buf();
        let roots = read_roots_from_file(&path)?;

        {
            let mut stored_path = self
                .storage_path
                .lock()
                .map_err(|_| "managed root storage path lock poisoned".to_string())?;
            *stored_path = Some(path);
        }

        let mut stored_roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;
        *stored_roots = roots
            .into_iter()
            .map(|root| (root.root_id.clone(), root))
            .collect();

        Ok(())
    }

    pub fn upsert(&self, root: ManagedRoot) -> Result<ManagedRoot, String> {
        let snapshot = {
            let mut roots = self
                .roots
                .lock()
                .map_err(|_| "managed root store lock poisoned".to_string())?;

            roots.insert(root.root_id.clone(), root.clone());
            roots.values().cloned().collect::<Vec<_>>()
        };

        self.save_snapshot(&snapshot)?;
        Ok(root)
    }

    pub fn list(&self) -> Result<Vec<ManagedRoot>, String> {
        let roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;

        Ok(roots.values().cloned().collect())
    }

    pub fn get(&self, root_id: &str) -> Result<ManagedRoot, String> {
        let roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;

        roots
            .get(root_id)
            .cloned()
            .ok_or_else(|| format!("managed root is not registered: {root_id}"))
    }

    pub fn contains_root(&self, root: &str) -> Result<bool, String> {
        let roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;

        Ok(roots.values().any(|managed| managed.root == root))
    }

    fn save_snapshot(&self, roots: &[ManagedRoot]) -> Result<(), String> {
        let path = self
            .storage_path
            .lock()
            .map_err(|_| "managed root storage path lock poisoned".to_string())?
            .clone();

        if let Some(path) = path {
            write_roots_to_file(&path, roots)?;
        }

        Ok(())
    }
}

fn read_roots_from_file(path: &Path) -> Result<Vec<ManagedRoot>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)
        .map_err(|error| format!("cannot read managed roots {}: {error}", path.display()))?;

    serde_json::from_str(&content)
        .map_err(|error| format!("cannot parse managed roots {}: {error}", path.display()))
}

fn write_roots_to_file(path: &Path, roots: &[ManagedRoot]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "cannot create managed root storage directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let content = serde_json::to_string_pretty(roots)
        .map_err(|error| format!("cannot serialize managed roots: {error}"))?;
    let temp_path = path.with_extension("json.tmp");

    fs::write(&temp_path, content).map_err(|error| {
        format!(
            "cannot write managed roots temp file {}: {error}",
            temp_path.display()
        )
    })?;
    fs::rename(&temp_path, path).map_err(|error| {
        format!(
            "cannot replace managed roots file {} with {}: {error}",
            path.display(),
            temp_path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{ManagedRoot, ManagedRootStore};

    #[test]
    fn upsert_keeps_one_entry_per_root_id() {
        let store = ManagedRootStore::default();

        store
            .upsert(ManagedRoot {
                root_id: "root:cafe".to_string(),
                root: "C:/work".to_string(),
                display_name: "work".to_string(),
            })
            .expect("insert root");
        store
            .upsert(ManagedRoot {
                root_id: "root:cafe".to_string(),
                root: "C:/work".to_string(),
                display_name: "renamed".to_string(),
            })
            .expect("update root");

        let roots = store.list().expect("list roots");

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].display_name, "renamed");
    }

    #[test]
    fn upsert_persists_roots_after_loading_storage_path() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("managed-roots.json");
        let store = ManagedRootStore::default();

        store.load_from_file(&path).expect("configure storage path");
        store
            .upsert(ManagedRoot {
                root_id: "root:cafe".to_string(),
                root: "C:/work".to_string(),
                display_name: "work".to_string(),
            })
            .expect("insert root");

        let content = fs::read_to_string(&path).expect("read stored roots");
        let stored = serde_json::from_str::<Vec<ManagedRoot>>(&content).expect("parse roots");

        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].root, "C:/work");
    }

    #[test]
    fn load_from_file_restores_roots() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("managed-roots.json");
        fs::write(
            &path,
            r#"[{"root_id":"root:cafe","root":"C:/work","display_name":"work"}]"#,
        )
        .expect("write roots");
        let store = ManagedRootStore::default();

        store.load_from_file(&path).expect("load roots");

        assert!(store.contains_root("C:/work").expect("contains root"));
        assert_eq!(
            store.get("root:cafe").expect("get root").display_name,
            "work"
        );
    }
}
