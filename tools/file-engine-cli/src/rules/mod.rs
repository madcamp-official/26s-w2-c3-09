mod extension;

use std::collections::HashSet;

use crate::analyzer::FileEntry;
use crate::proposal::Proposal;

pub struct RuleContext {
    pub existing_paths: HashSet<String>,
}

pub trait ProposalRule {
    fn propose(&self, file: &FileEntry, context: &RuleContext) -> Option<Proposal>;
}

pub fn default_rules() -> Vec<Box<dyn ProposalRule>> {
    vec![Box::new(extension::ExtensionRule)]
}

pub fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}
