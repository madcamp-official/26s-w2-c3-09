use std::path::{Path, PathBuf};

use crate::analyzer::FileEntry;
use crate::proposal::{Proposal, ProposalAction, ProposalStatus};
use crate::rules::{normalize_relative_path, ProposalRule, RuleContext};

pub struct ExtensionRule;

impl ProposalRule for ExtensionRule {
    fn propose(&self, file: &FileEntry, context: &RuleContext) -> Option<Proposal> {
        let category = category_for_path(&file.path)?;
        let file_name = Path::new(&file.path).file_name()?.to_string_lossy();
        let target = format!("{category}/{file_name}");
        let normalized_target = normalize_relative_path(&target);

        if normalize_relative_path(&file.path) == normalized_target {
            return None;
        }

        let status = if context.existing_paths.contains(&normalized_target) {
            ProposalStatus::DestinationExists
        } else {
            ProposalStatus::Ready
        };

        Some(Proposal {
            action: ProposalAction::Move,
            from: file.path.clone(),
            to: target,
            reason: format!(
                "{} files belong in {category}/",
                extension_label(&file.path)
            ),
            status,
        })
    }
}

fn category_for_path(path: &str) -> Option<&'static str> {
    match extension(path)?.as_str() {
        "md" | "pdf" | "doc" | "docx" | "txt" => Some("documents"),
        "png" | "jpg" | "jpeg" | "gif" | "webp" => Some("images"),
        "zip" | "tar" | "gz" | "7z" => Some("archives"),
        _ => None,
    }
}

fn extension_label(path: &str) -> String {
    extension(path)
        .map(|extension| format!(".{extension}"))
        .unwrap_or_else(|| "matched".to_string())
}

fn extension(path: &str) -> Option<String> {
    PathBuf::from(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}
