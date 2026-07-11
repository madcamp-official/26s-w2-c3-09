use std::collections::BTreeMap;
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
}

impl ManagedRootStore {
    pub fn upsert(&self, root: ManagedRoot) -> Result<ManagedRoot, String> {
        let mut roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;

        roots.insert(root.root_id.clone(), root.clone());
        Ok(root)
    }

    pub fn list(&self) -> Result<Vec<ManagedRoot>, String> {
        let roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;

        Ok(roots.values().cloned().collect())
    }

    pub fn contains_root(&self, root: &str) -> Result<bool, String> {
        let roots = self
            .roots
            .lock()
            .map_err(|_| "managed root store lock poisoned".to_string())?;

        Ok(roots.values().any(|managed| managed.root == root))
    }
}

#[cfg(test)]
mod tests {
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
}
