mod definition;
mod glob;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::analyzer::FileEntry;
use crate::journal::{STATE_DIR, TRASH_DIR};
use crate::proposal::{proposal_id, Proposal, ProposalAction, ProposalStatus};
use serde_json::Value;

pub use definition::{
    Action, Condition, FileKind, Rule, RuleError, RuleSet, CURRENT_RULES_VERSION,
};

pub const RULES_FILE: &str = "rules.json";
const DAY_MS: u128 = 24 * 60 * 60 * 1000;

/// Accepts either MouseKeeper's root-local `rules.json` shape or the server/mobile
/// `RuleDefinition` contract shape. Draft rules can therefore be validated locally before
/// they ever produce concrete proposals.
pub fn rule_set_from_draft_value(value: Value) -> Result<RuleSet, RuleError> {
    match serde_json::from_value::<RuleSet>(value.clone()) {
        Ok(set) => {
            set.validate()?;
            Ok(set)
        }
        Err(internal_error) => RuleSet::from_contract_definition("draft-rule", 0, value).map_err(
            |contract_error| {
                RuleError::ParseContract(format!(
                    "internal RuleSet parse failed: {internal_error}; contract RuleDefinition parse failed: {contract_error}"
                ))
            },
        ),
    }
}

pub struct RuleContext {
    pub existing_paths: HashSet<String>,
    /// Current wall-clock time, injected so `older_than_days` is deterministic in tests
    /// instead of reading the system clock deep inside the evaluator.
    pub now_unix_ms: u128,
}

impl RuleSet {
    /// Evaluates rules against one file in priority order (lower `priority` first, ties broken
    /// by `id`) and returns the first fully-matching rule's proposal. "First match wins" keeps
    /// the outcome deterministic when several user/LLM rules could touch the same file.
    pub fn propose(&self, file: &FileEntry, context: &RuleContext) -> Option<Proposal> {
        let mut ordered: Vec<&Rule> = self.rules.iter().collect();
        ordered.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.id.cmp(&right.id))
        });

        ordered
            .into_iter()
            .find_map(|rule| proposal_from_rule(rule, file, context))
    }
}

fn proposal_from_rule(rule: &Rule, file: &FileEntry, context: &RuleContext) -> Option<Proposal> {
    if !rule_matches(rule, file, context) {
        return None;
    }

    match (&rule.then.move_to, rule.then.trash, &rule.then.create_dir) {
        (Some(move_to), false, None) => move_proposal_from_rule(rule, file, context, move_to),
        (None, true, None) => trash_proposal_from_rule(rule, file),
        (None, false, Some(create_dir)) => create_dir_proposal_from_rule(rule, context, create_dir),
        _ => None,
    }
}

fn move_proposal_from_rule(
    rule: &Rule,
    file: &FileEntry,
    context: &RuleContext,
    move_to: &str,
) -> Option<Proposal> {
    let file_name = Path::new(&file.path).file_name()?.to_string_lossy();
    let target = format!("{move_to}/{file_name}");
    let normalized_target = normalize_relative_path(&target);

    // The file is already where the rule would move it; nothing to propose.
    if normalize_relative_path(&file.path) == normalized_target {
        return None;
    }

    let status = if context.existing_paths.contains(&normalized_target) {
        ProposalStatus::DestinationExists
    } else {
        ProposalStatus::Ready
    };

    let action = ProposalAction::Move;

    Some(Proposal {
        proposal_id: proposal_id(&action, &file.path, &target),
        action,
        from: file.path.clone(),
        to: target,
        content: None,
        source_size_bytes: file.size_bytes,
        source_modified_unix_ms: file.modified_unix_ms,
        source_file_id: file.file_id.clone(),
        reason: reason_for(rule, file),
        status,
    })
}

fn trash_proposal_from_rule(rule: &Rule, file: &FileEntry) -> Option<Proposal> {
    let action = ProposalAction::Trash;
    let target = TRASH_DIR.to_string();

    Some(Proposal {
        proposal_id: proposal_id(&action, &file.path, &target),
        action,
        from: file.path.clone(),
        to: target,
        content: None,
        source_size_bytes: file.size_bytes,
        source_modified_unix_ms: file.modified_unix_ms,
        source_file_id: file.file_id.clone(),
        reason: reason_for(rule, file),
        status: ProposalStatus::Ready,
    })
}

fn create_dir_proposal_from_rule(
    rule: &Rule,
    context: &RuleContext,
    target: &str,
) -> Option<Proposal> {
    let action = ProposalAction::CreateDir;
    let normalized_target = normalize_relative_path(target);
    let status = if context.existing_paths.contains(&normalized_target) {
        ProposalStatus::DestinationExists
    } else {
        ProposalStatus::Ready
    };

    Some(Proposal {
        proposal_id: proposal_id(&action, "", target),
        action,
        from: String::new(),
        to: target.to_string(),
        content: None,
        source_size_bytes: 0,
        source_modified_unix_ms: None,
        source_file_id: None,
        reason: format!("rule '{}' creates this managed-root directory", rule.id),
        status,
    })
}

fn rule_matches(rule: &Rule, file: &FileEntry, context: &RuleContext) -> bool {
    let condition = &rule.when;

    if let Some(extensions) = &condition.extension_in {
        match extension_of(&file.path) {
            Some(extension)
                if extensions
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(&extension)) => {}
            _ => return false,
        }
    }

    if let Some(days) = condition.older_than_days {
        match file.modified_unix_ms {
            // A file with no known modified time cannot be proven old, so it does not match.
            Some(modified) => {
                let threshold = u128::from(days) * DAY_MS;
                if context.now_unix_ms.saturating_sub(modified) < threshold {
                    return false;
                }
            }
            None => return false,
        }
    }

    if let Some(days) = condition.modified_age_days_gte {
        if !age_days_matches(
            file.modified_unix_ms,
            context.now_unix_ms,
            days,
            AgeCompare::Gte,
        ) {
            return false;
        }
    }

    if let Some(days) = condition.modified_age_days_gt {
        if !age_days_matches(
            file.modified_unix_ms,
            context.now_unix_ms,
            days,
            AgeCompare::Gt,
        ) {
            return false;
        }
    }

    if let Some(days) = condition.created_age_days_gte {
        if !age_days_matches(
            file.created_unix_ms,
            context.now_unix_ms,
            days,
            AgeCompare::Gte,
        ) {
            return false;
        }
    }

    if let Some(days) = condition.created_age_days_gt {
        if !age_days_matches(
            file.created_unix_ms,
            context.now_unix_ms,
            days,
            AgeCompare::Gt,
        ) {
            return false;
        }
    }

    if let Some(min_size) = condition.size_bytes_gte {
        if file.size_bytes < min_size {
            return false;
        }
    }

    if let Some(max_size) = condition.size_bytes_lte {
        if file.size_bytes > max_size {
            return false;
        }
    }

    if let Some(prefix) = &condition.relative_path_starts_with {
        if !relative_path_has_prefix(&file.path, prefix) {
            return false;
        }
    }

    if let Some(kind) = condition.file_kind {
        if kind != FileKind::File {
            return false;
        }
    }

    if let Some(pattern) = &condition.name_matches {
        match Path::new(&file.path)
            .file_name()
            .and_then(|name| name.to_str())
        {
            Some(file_name) if glob::glob_match(pattern, file_name) => {}
            _ => return false,
        }
    }

    if let Some(needle) = &condition.name_contains {
        if !file_name(&file.path).is_some_and(|name| contains_ignore_case(name, needle)) {
            return false;
        }
    }

    if let Some(prefix) = &condition.name_starts_with {
        if !file_name(&file.path).is_some_and(|name| starts_with_ignore_case(name, prefix)) {
            return false;
        }
    }

    if let Some(suffix) = &condition.name_ends_with {
        if !file_name(&file.path).is_some_and(|name| ends_with_ignore_case(name, suffix)) {
            return false;
        }
    }

    true
}

fn reason_for(rule: &Rule, file: &FileEntry) -> String {
    if rule.then.trash {
        return format!("rule '{}' moves this into recoverable trash", rule.id);
    }

    let move_to = rule.then.move_to.as_deref().unwrap_or("destination");
    match (rule.when.extension_in.is_some(), extension_of(&file.path)) {
        (true, Some(extension)) => format!(".{extension} files belong in {move_to}/"),
        _ => format!("rule '{}' moves this into {move_to}/", rule.id),
    }
}

fn extension_of(path: &str) -> Option<String> {
    PathBuf::from(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

#[derive(Clone, Copy)]
enum AgeCompare {
    Gte,
    Gt,
}

fn age_days_matches(
    timestamp_unix_ms: Option<u128>,
    now_unix_ms: u128,
    days: u32,
    compare: AgeCompare,
) -> bool {
    let Some(timestamp_unix_ms) = timestamp_unix_ms else {
        return false;
    };
    let age = now_unix_ms.saturating_sub(timestamp_unix_ms);
    let threshold = u128::from(days) * DAY_MS;
    match compare {
        AgeCompare::Gte => age >= threshold,
        AgeCompare::Gt => age > threshold,
    }
}

fn relative_path_has_prefix(path: &str, prefix: &str) -> bool {
    let path = normalize_relative_path(path).trim_matches('/').to_string();
    let prefix = normalize_relative_path(prefix)
        .trim_matches('/')
        .to_string();
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

fn file_name(path: &str) -> Option<&str> {
    Path::new(path).file_name().and_then(|name| name.to_str())
}

fn contains_ignore_case(value: &str, needle: &str) -> bool {
    value
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn starts_with_ignore_case(value: &str, prefix: &str) -> bool {
    value
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
}

fn ends_with_ignore_case(value: &str, suffix: &str) -> bool {
    value
        .to_ascii_lowercase()
        .ends_with(&suffix.to_ascii_lowercase())
}

/// The rule set applied when a root has no `rules.json`. It reproduces the original hardcoded
/// extension buckets so behavior is unchanged out of the box; a root opts into custom behavior
/// only by writing its own file.
pub fn default_rule_set() -> RuleSet {
    fn rule(id: &str, extensions: &[&str], move_to: &str) -> Rule {
        Rule {
            id: id.to_string(),
            priority: 10,
            when: Condition {
                extension_in: Some(extensions.iter().map(|value| value.to_string()).collect()),
                ..Condition::default()
            },
            then: Action {
                move_to: Some(move_to.to_string()),
                trash: false,
                create_dir: None,
            },
        }
    }

    RuleSet {
        version: CURRENT_RULES_VERSION,
        rules: vec![
            rule(
                "documents",
                &["md", "pdf", "doc", "docx", "txt"],
                "documents",
            ),
            rule("images", &["png", "jpg", "jpeg", "gif", "webp"], "images"),
            rule("archives", &["zip", "tar", "gz", "7z"], "archives"),
        ],
    }
}

/// Loads a root's rule set from `.mousekeeper/rules.json`, falling back to the default preset
/// when the file is absent. A present-but-invalid file is a hard error: we refuse to silently
/// ignore user/LLM rules and fall back, since that would hide a real misconfiguration.
pub fn load_rule_set_for_root(root: impl AsRef<Path>) -> Result<RuleSet, RuleError> {
    let path = root.as_ref().join(STATE_DIR).join(RULES_FILE);
    if !path.exists() {
        return Ok(default_rule_set());
    }

    let content = fs::read_to_string(&path).map_err(|error| RuleError::Read {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let content = content.trim_start_matches('\u{feff}');

    let set = serde_json::from_str::<RuleSet>(content).map_err(|error| RuleError::Parse {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    set.validate()?;

    Ok(set)
}

pub fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::analyzer::FileEntry;

    use super::{
        default_rule_set, load_rule_set_for_root, Action, Condition, Rule, RuleContext, RuleSet,
        CURRENT_RULES_VERSION, DAY_MS,
    };

    fn context(now_unix_ms: u128) -> RuleContext {
        RuleContext {
            existing_paths: HashSet::new(),
            now_unix_ms,
        }
    }

    fn file(path: &str, modified_unix_ms: Option<u128>) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            size_bytes: 10,
            modified_unix_ms,
            created_unix_ms: None,
            file_id: None,
        }
    }

    #[test]
    fn default_rule_set_moves_by_extension() {
        let rules = default_rule_set();
        let proposal = rules
            .propose(&file("inbox/note.md", None), &context(0))
            .expect("proposal");

        assert_eq!(proposal.from, "inbox/note.md");
        assert_eq!(proposal.to, "documents/note.md");
    }

    #[test]
    fn trash_rule_proposes_recoverable_trash_action() {
        let rules = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![Rule {
                id: "old-temp".to_string(),
                priority: 0,
                when: Condition {
                    name_matches: Some("*.tmp".to_string()),
                    ..Condition::default()
                },
                then: Action {
                    move_to: None,
                    trash: true,
                    create_dir: None,
                },
            }],
        };
        let proposal = rules
            .propose(&file("inbox/cache.tmp", None), &context(0))
            .expect("proposal");

        assert_eq!(proposal.action, crate::proposal::ProposalAction::Trash);
        assert_eq!(proposal.to, crate::journal::TRASH_DIR);
        assert!(proposal.proposal_id.starts_with("trash:"));
    }

    #[test]
    fn older_than_days_only_matches_sufficiently_old_files() {
        let now = 100 * DAY_MS;
        let rules = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![Rule {
                id: "archive-old".to_string(),
                priority: 0,
                when: Condition {
                    older_than_days: Some(30),
                    ..Condition::default()
                },
                then: Action {
                    move_to: Some("old".to_string()),
                    trash: false,
                    create_dir: None,
                },
            }],
        };

        let fresh = file("inbox/recent.txt", Some(now - 5 * DAY_MS));
        assert!(rules.propose(&fresh, &context(now)).is_none());

        let stale = file("inbox/ancient.txt", Some(now - 60 * DAY_MS));
        let proposal = rules.propose(&stale, &context(now)).expect("proposal");
        assert_eq!(proposal.to, "old/ancient.txt");
    }

    #[test]
    fn name_matches_uses_glob_pattern() {
        let rules = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![Rule {
                id: "invoices".to_string(),
                priority: 0,
                when: Condition {
                    name_matches: Some("*invoice*".to_string()),
                    ..Condition::default()
                },
                then: Action {
                    move_to: Some("billing".to_string()),
                    trash: false,
                    create_dir: None,
                },
            }],
        };

        let matched = file("inbox/2024-invoice.pdf", None);
        assert_eq!(
            rules.propose(&matched, &context(0)).expect("proposal").to,
            "billing/2024-invoice.pdf"
        );

        let unmatched = file("inbox/photo.png", None);
        assert!(rules.propose(&unmatched, &context(0)).is_none());
    }

    #[test]
    fn conditions_are_anded_together() {
        let now = 100 * DAY_MS;
        let rules = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![Rule {
                id: "old-pdfs".to_string(),
                priority: 0,
                when: Condition {
                    extension_in: Some(vec!["pdf".to_string()]),
                    older_than_days: Some(30),
                    ..Condition::default()
                },
                then: Action {
                    move_to: Some("archive".to_string()),
                    trash: false,
                    create_dir: None,
                },
            }],
        };

        // Right extension but too new: no match because both conditions must hold.
        let new_pdf = file("inbox/new.pdf", Some(now - 1 * DAY_MS));
        assert!(rules.propose(&new_pdf, &context(now)).is_none());

        let old_pdf = file("inbox/old.pdf", Some(now - 90 * DAY_MS));
        assert!(rules.propose(&old_pdf, &context(now)).is_some());
    }

    #[test]
    fn first_matching_rule_wins_by_priority() {
        let rules = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![
                Rule {
                    id: "specific".to_string(),
                    priority: 1,
                    when: Condition {
                        name_matches: Some("secret*".to_string()),
                        ..Condition::default()
                    },
                    then: Action {
                        move_to: Some("vault".to_string()),
                        trash: false,
                        create_dir: None,
                    },
                },
                Rule {
                    id: "general".to_string(),
                    priority: 5,
                    when: Condition {
                        extension_in: Some(vec!["md".to_string()]),
                        ..Condition::default()
                    },
                    then: Action {
                        move_to: Some("documents".to_string()),
                        trash: false,
                        create_dir: None,
                    },
                },
            ],
        };

        // Matches both rules; the lower-priority-number rule (vault) should win.
        let proposal = rules
            .propose(&file("inbox/secret.md", None), &context(0))
            .expect("proposal");
        assert_eq!(proposal.to, "vault/secret.md");
    }

    #[test]
    fn missing_rules_file_falls_back_to_default() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();

        let rules = load_rule_set_for_root(root).expect("load default");
        assert_eq!(rules, default_rule_set());
    }

    #[test]
    fn loads_and_validates_custom_rules_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let state_dir = root.join(".mousekeeper");
        std::fs::create_dir_all(&state_dir).expect("create state dir");
        std::fs::write(
            state_dir.join("rules.json"),
            r#"{"version":1,"rules":[{"id":"pics","when":{"extension_in":["png"]},"then":{"move_to":"pictures"}}]}"#,
        )
        .expect("write rules");

        let rules = load_rule_set_for_root(root).expect("load custom rules");
        assert_eq!(rules.rules.len(), 1);
        assert_eq!(rules.rules[0].then.move_to.as_deref(), Some("pictures"));
    }

    #[test]
    fn invalid_rules_file_is_a_hard_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let state_dir = root.join(".mousekeeper");
        std::fs::create_dir_all(&state_dir).expect("create state dir");
        std::fs::write(
            state_dir.join("rules.json"),
            r#"{"version":1,"rules":[{"id":"bad","when":{"extension_in":["pdf"]},"then":{"move_to":"../escape"}}]}"#,
        )
        .expect("write rules");

        assert!(load_rule_set_for_root(root).is_err());
    }
}
