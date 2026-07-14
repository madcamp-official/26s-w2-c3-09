use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Bumped only on breaking changes to the rule schema. Loading a rule set with a different
/// version is rejected rather than silently reinterpreted, so an old file cannot be misread
/// as a newer shape.
pub const CURRENT_RULES_VERSION: u32 = 1;

/// A user- or LLM-authored set of file-organizing rules. This is data, not code: it is the
/// whole point of the DSL that a rule can be added by writing JSON (from a natural-language
/// request translated server-side, or a per-user file) without recompiling the engine.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuleSet {
    pub version: u32,
    pub rules: Vec<Rule>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    /// Lower runs first; ties broken by `id`. The first fully-matching rule wins for a file.
    #[serde(default)]
    pub priority: i64,
    pub when: Condition,
    pub then: Action,
}

/// All present conditions must match (logical AND). A rule with no conditions is rejected at
/// validation time so a malformed/empty rule can never mean "move everything".
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_in: Option<Vec<String>>,
    /// Backward-compatible alias for `modified_age_days_gte`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub older_than_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_age_days_gte: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_age_days_gt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_age_days_gte: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_age_days_gt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes_gte: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes_lte: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_path_starts_with: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_kind: Option<FileKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_matches: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_contains: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_starts_with: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_ends_with: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FileKind {
    File,
    Directory,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Action {
    /// Destination directory relative to the managed root. Validated to stay inside the root
    /// (no absolute paths, no `..`) because this value can come from an untrusted source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_to: Option<String>,
    /// Moves a matched file into MouseKeeper's recoverable root-local trash.
    #[serde(default, skip_serializing_if = "is_false")]
    pub trash: bool,
    /// Creates one empty directory proposal relative to the managed root. This still only creates
    /// a proposal; journaled filesystem writes happen later, after explicit user approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_dir: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RuleError {
    UnsupportedVersion {
        found: u32,
        supported: u32,
    },
    EmptyRuleId,
    DuplicateRuleId(String),
    EmptyCondition(String),
    EmptyExtensionList(String),
    InvalidExtension {
        rule: String,
        extension: String,
    },
    EmptyNamePattern(String),
    InvalidTarget {
        rule: String,
        target: String,
        message: String,
    },
    InvalidCondition {
        rule: String,
        condition: String,
        message: String,
    },
    ParseContract(String),
    Read {
        path: String,
        message: String,
    },
    Parse {
        path: String,
        message: String,
    },
    Serialize(String),
}

impl RuleSet {
    pub fn validate(&self) -> Result<(), RuleError> {
        if self.version != CURRENT_RULES_VERSION {
            return Err(RuleError::UnsupportedVersion {
                found: self.version,
                supported: CURRENT_RULES_VERSION,
            });
        }

        let mut seen_ids = HashSet::new();
        for rule in &self.rules {
            rule.validate()?;
            if !seen_ids.insert(rule.id.as_str()) {
                return Err(RuleError::DuplicateRuleId(rule.id.clone()));
            }
        }

        Ok(())
    }

    pub fn from_contract_definition(
        rule_id: impl Into<String>,
        priority: i64,
        definition: Value,
    ) -> Result<Self, RuleError> {
        let rule_id = rule_id.into();
        let definition = serde_json::from_value::<ContractRuleDefinition>(definition)
            .map_err(|error| RuleError::ParseContract(error.to_string()))?;
        definition.into_rule_set(rule_id, priority)
    }
}

impl Rule {
    fn validate(&self) -> Result<(), RuleError> {
        if self.id.trim().is_empty() {
            return Err(RuleError::EmptyRuleId);
        }

        if self.when.is_empty() {
            return Err(RuleError::EmptyCondition(self.id.clone()));
        }

        if let Some(extensions) = &self.when.extension_in {
            if extensions.is_empty() {
                return Err(RuleError::EmptyExtensionList(self.id.clone()));
            }
            for extension in extensions {
                if extension.trim().is_empty() || extension.contains('.') {
                    return Err(RuleError::InvalidExtension {
                        rule: self.id.clone(),
                        extension: extension.clone(),
                    });
                }
            }
        }

        if let Some(pattern) = &self.when.name_matches {
            if pattern.trim().is_empty() {
                return Err(RuleError::EmptyNamePattern(self.id.clone()));
            }
        }
        for (label, value) in [
            ("name_contains", &self.when.name_contains),
            ("name_starts_with", &self.when.name_starts_with),
            ("name_ends_with", &self.when.name_ends_with),
        ] {
            if value
                .as_ref()
                .is_some_and(|pattern| pattern.trim().is_empty())
            {
                return Err(RuleError::InvalidCondition {
                    rule: self.id.clone(),
                    condition: label.to_string(),
                    message: "name condition cannot be empty".to_string(),
                });
            }
        }
        if let Some(prefix) = &self.when.relative_path_starts_with {
            validate_relative_path_prefix(prefix).map_err(|message| {
                RuleError::InvalidCondition {
                    rule: self.id.clone(),
                    condition: "relative_path_starts_with".to_string(),
                    message,
                }
            })?;
        }

        let action_count = usize::from(self.then.move_to.is_some())
            + usize::from(self.then.trash)
            + usize::from(self.then.create_dir.is_some());
        if action_count != 1 {
            return Err(RuleError::InvalidTarget {
                rule: self.id.clone(),
                target: "action".to_string(),
                message: "choose exactly one action".to_string(),
            });
        }

        if let Some(target) = &self.then.move_to {
            validate_relative_dir(target).map_err(|message| RuleError::InvalidTarget {
                rule: self.id.clone(),
                target: target.clone(),
                message,
            })?;
        }
        if let Some(target) = &self.then.create_dir {
            validate_relative_dir(target).map_err(|message| RuleError::InvalidTarget {
                rule: self.id.clone(),
                target: target.clone(),
                message,
            })?;
        }

        Ok(())
    }
}

impl Condition {
    fn is_empty(&self) -> bool {
        self.extension_in.is_none()
            && self.older_than_days.is_none()
            && self.modified_age_days_gte.is_none()
            && self.modified_age_days_gt.is_none()
            && self.created_age_days_gte.is_none()
            && self.created_age_days_gt.is_none()
            && self.size_bytes_gte.is_none()
            && self.size_bytes_lte.is_none()
            && self.relative_path_starts_with.is_none()
            && self.file_kind.is_none()
            && self.name_matches.is_none()
            && self.name_contains.is_none()
            && self.name_starts_with.is_none()
            && self.name_ends_with.is_none()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContractRuleDefinition {
    #[serde(default, rename = "match")]
    match_mode: ContractMatchMode,
    conditions: Vec<ContractCondition>,
    action: ContractAction,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ContractMatchMode {
    #[default]
    All,
    Any,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
#[serde(tag = "field")]
enum ContractCondition {
    #[serde(rename = "extension")]
    Extension {
        operator: ContractInOperator,
        value: Vec<String>,
    },
    #[serde(rename = "ageDays")]
    AgeDays {
        operator: ContractGteOperator,
        value: u32,
    },
    #[serde(rename = "modifiedAgeDays")]
    ModifiedAgeDays {
        operator: ContractAgeOperator,
        value: u32,
    },
    #[serde(rename = "createdAgeDays")]
    CreatedAgeDays {
        operator: ContractAgeOperator,
        value: u32,
    },
    #[serde(rename = "sizeBytes")]
    SizeBytes {
        operator: ContractSizeOperator,
        value: u64,
    },
    #[serde(rename = "relativePath")]
    RelativePath {
        operator: ContractStartsWithOperator,
        value: String,
    },
    #[serde(rename = "fileKind")]
    FileKind {
        operator: ContractEqOperator,
        value: FileKind,
    },
    #[serde(rename = "name")]
    Name {
        operator: ContractNameOperator,
        value: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum ContractInOperator {
    #[serde(rename = "IN")]
    In,
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum ContractGteOperator {
    #[serde(rename = "GTE")]
    Gte,
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum ContractAgeOperator {
    #[serde(rename = "GTE")]
    Gte,
    #[serde(rename = "GT")]
    Gt,
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum ContractSizeOperator {
    #[serde(rename = "GTE")]
    Gte,
    #[serde(rename = "LTE")]
    Lte,
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum ContractStartsWithOperator {
    #[serde(rename = "STARTS_WITH")]
    StartsWith,
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum ContractEqOperator {
    #[serde(rename = "EQ")]
    Eq,
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum ContractNameOperator {
    #[serde(rename = "CONTAINS")]
    Contains,
    #[serde(rename = "STARTS_WITH")]
    StartsWith,
    #[serde(rename = "ENDS_WITH")]
    EndsWith,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
enum ContractAction {
    #[serde(rename = "MOVE")]
    Move {
        #[serde(rename = "destinationTemplate")]
        destination_template: String,
    },
    #[serde(rename = "QUARANTINE")]
    Quarantine,
    #[serde(rename = "TRASH")]
    Trash,
    #[serde(rename = "CREATE_DIR")]
    CreateDir {
        #[serde(rename = "relativePath")]
        relative_path: String,
    },
}

impl ContractRuleDefinition {
    fn into_rule_set(self, rule_id: String, priority: i64) -> Result<RuleSet, RuleError> {
        let action = self.action.into_action();
        let rules = match self.match_mode {
            ContractMatchMode::All => {
                let mut condition = Condition::default();
                for contract_condition in self.conditions {
                    condition.apply_contract_condition(&rule_id, contract_condition)?;
                }
                vec![Rule {
                    id: rule_id,
                    priority,
                    when: condition,
                    then: action,
                }]
            }
            ContractMatchMode::Any => self
                .conditions
                .into_iter()
                .enumerate()
                .map(|(index, contract_condition)| {
                    let id = format!("{rule_id}-any-{}", index + 1);
                    let mut condition = Condition::default();
                    condition.apply_contract_condition(&id, contract_condition)?;
                    Ok(Rule {
                        id,
                        priority,
                        when: condition,
                        then: action.clone(),
                    })
                })
                .collect::<Result<Vec<_>, RuleError>>()?,
        };

        let set = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules,
        };
        set.validate()?;
        Ok(set)
    }
}

impl ContractAction {
    fn into_action(self) -> Action {
        match self {
            ContractAction::Move {
                destination_template,
            } => Action {
                move_to: Some(destination_template),
                trash: false,
                create_dir: None,
            },
            ContractAction::Quarantine | ContractAction::Trash => Action {
                move_to: None,
                trash: true,
                create_dir: None,
            },
            ContractAction::CreateDir { relative_path } => Action {
                move_to: None,
                trash: false,
                create_dir: Some(relative_path),
            },
        }
    }
}

impl Condition {
    fn apply_contract_condition(
        &mut self,
        rule_id: &str,
        condition: ContractCondition,
    ) -> Result<(), RuleError> {
        match condition {
            ContractCondition::Extension { operator: _, value } => set_once(
                rule_id,
                "extension_in",
                &mut self.extension_in,
                value
                    .into_iter()
                    .map(|extension| normalize_contract_extension(rule_id, extension.as_str()))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            ContractCondition::AgeDays { operator: _, value } => {
                set_once(rule_id, "older_than_days", &mut self.older_than_days, value)
            }
            ContractCondition::ModifiedAgeDays { operator, value } => match operator {
                ContractAgeOperator::Gte => set_once(
                    rule_id,
                    "modified_age_days_gte",
                    &mut self.modified_age_days_gte,
                    value,
                ),
                ContractAgeOperator::Gt => set_once(
                    rule_id,
                    "modified_age_days_gt",
                    &mut self.modified_age_days_gt,
                    value,
                ),
            },
            ContractCondition::CreatedAgeDays { operator, value } => match operator {
                ContractAgeOperator::Gte => set_once(
                    rule_id,
                    "created_age_days_gte",
                    &mut self.created_age_days_gte,
                    value,
                ),
                ContractAgeOperator::Gt => set_once(
                    rule_id,
                    "created_age_days_gt",
                    &mut self.created_age_days_gt,
                    value,
                ),
            },
            ContractCondition::SizeBytes { operator, value } => match operator {
                ContractSizeOperator::Gte => {
                    set_once(rule_id, "size_bytes_gte", &mut self.size_bytes_gte, value)
                }
                ContractSizeOperator::Lte => {
                    set_once(rule_id, "size_bytes_lte", &mut self.size_bytes_lte, value)
                }
            },
            ContractCondition::RelativePath { operator: _, value } => set_once(
                rule_id,
                "relative_path_starts_with",
                &mut self.relative_path_starts_with,
                value,
            ),
            ContractCondition::FileKind { operator: _, value } => {
                set_once(rule_id, "file_kind", &mut self.file_kind, value)
            }
            ContractCondition::Name { operator, value } => match operator {
                ContractNameOperator::Contains => {
                    set_once(rule_id, "name_contains", &mut self.name_contains, value)
                }
                ContractNameOperator::StartsWith => set_once(
                    rule_id,
                    "name_starts_with",
                    &mut self.name_starts_with,
                    value,
                ),
                ContractNameOperator::EndsWith => {
                    set_once(rule_id, "name_ends_with", &mut self.name_ends_with, value)
                }
            },
        }
    }
}

fn set_once<T>(
    rule_id: &str,
    condition: &str,
    target: &mut Option<T>,
    value: T,
) -> Result<(), RuleError> {
    if target.is_some() {
        return Err(RuleError::InvalidCondition {
            rule: rule_id.to_string(),
            condition: condition.to_string(),
            message: "duplicate condition is ambiguous".to_string(),
        });
    }
    *target = Some(value);
    Ok(())
}

fn normalize_contract_extension(rule_id: &str, extension: &str) -> Result<String, RuleError> {
    let trimmed = extension.trim();
    let bare = trimmed.strip_prefix('.').unwrap_or(trimmed);
    if bare.is_empty() || bare.contains('.') {
        return Err(RuleError::InvalidExtension {
            rule: rule_id.to_string(),
            extension: extension.to_string(),
        });
    }
    Ok(bare.to_ascii_lowercase())
}

/// Rejects a `move_to` that is empty, absolute, or escapes the managed root. This mirrors the
/// component rules `PathGuard` enforces, but runs at rule-definition time so a bad rule is
/// refused before it ever produces a proposal.
fn validate_relative_dir(target: &str) -> Result<(), String> {
    if target.trim().is_empty() {
        return Err("destination is empty".to_string());
    }
    if target.contains('\0') {
        return Err("destination cannot contain NUL".to_string());
    }

    let path = Path::new(target);
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                return Err("destination must be relative to the managed root".to_string());
            }
            Component::ParentDir => {
                return Err("destination cannot contain parent traversal".to_string());
            }
            Component::CurDir => {
                return Err("destination cannot contain current-directory segments".to_string());
            }
            Component::Normal(_) => {}
        }
    }

    Ok(())
}

fn validate_relative_path_prefix(prefix: &str) -> Result<(), String> {
    validate_relative_dir(prefix).map_err(|message| format!("prefix {message}"))
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl fmt::Display for RuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleError::UnsupportedVersion { found, supported } => {
                write!(
                    formatter,
                    "unsupported rules version {found}; this build supports version {supported}"
                )
            }
            RuleError::EmptyRuleId => write!(formatter, "rule id cannot be empty"),
            RuleError::DuplicateRuleId(id) => write!(formatter, "duplicate rule id: {id}"),
            RuleError::EmptyCondition(id) => {
                write!(formatter, "rule '{id}' must have at least one condition")
            }
            RuleError::EmptyExtensionList(id) => {
                write!(formatter, "rule '{id}' has an empty extension_in list")
            }
            RuleError::InvalidExtension { rule, extension } => {
                write!(
                    formatter,
                    "rule '{rule}' has an invalid extension '{extension}' (use bare extensions like \"pdf\")"
                )
            }
            RuleError::EmptyNamePattern(id) => {
                write!(formatter, "rule '{id}' has an empty name_matches pattern")
            }
            RuleError::InvalidTarget {
                rule,
                target,
                message,
            } => {
                write!(
                    formatter,
                    "rule '{rule}' has an invalid destination '{target}': {message}"
                )
            }
            RuleError::InvalidCondition {
                rule,
                condition,
                message,
            } => {
                write!(
                    formatter,
                    "rule '{rule}' has an invalid condition '{condition}': {message}"
                )
            }
            RuleError::ParseContract(message) => {
                write!(
                    formatter,
                    "cannot parse rule contract definition: {message}"
                )
            }
            RuleError::Read { path, message } => {
                write!(formatter, "cannot read rules {path}: {message}")
            }
            RuleError::Parse { path, message } => {
                write!(formatter, "cannot parse rules {path}: {message}")
            }
            RuleError::Serialize(message) => {
                write!(formatter, "cannot serialize rules: {message}")
            }
        }
    }
}

impl Error for RuleError {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Action, Condition, FileKind, Rule, RuleError, RuleSet, CURRENT_RULES_VERSION};

    fn rule(id: &str, when: Condition, move_to: &str) -> Rule {
        Rule {
            id: id.to_string(),
            priority: 0,
            when,
            then: Action {
                move_to: Some(move_to.to_string()),
                trash: false,
                create_dir: None,
            },
        }
    }

    fn ext_condition(extensions: &[&str]) -> Condition {
        Condition {
            extension_in: Some(extensions.iter().map(|value| value.to_string()).collect()),
            ..Condition::default()
        }
    }

    #[test]
    fn accepts_a_well_formed_rule_set() {
        let set = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![rule("docs", ext_condition(&["pdf", "md"]), "documents")],
        };

        assert!(set.validate().is_ok());
    }

    #[test]
    fn rejects_unsupported_version() {
        let set = RuleSet {
            version: CURRENT_RULES_VERSION + 1,
            rules: vec![],
        };

        assert!(matches!(
            set.validate(),
            Err(RuleError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn rejects_duplicate_rule_ids() {
        let set = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![
                rule("docs", ext_condition(&["pdf"]), "documents"),
                rule("docs", ext_condition(&["md"]), "documents"),
            ],
        };

        assert_eq!(
            set.validate(),
            Err(RuleError::DuplicateRuleId("docs".to_string()))
        );
    }

    #[test]
    fn rejects_rule_without_any_condition() {
        let set = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![rule("empty", Condition::default(), "documents")],
        };

        assert_eq!(
            set.validate(),
            Err(RuleError::EmptyCondition("empty".to_string()))
        );
    }

    #[test]
    fn rejects_extension_with_leading_dot() {
        let set = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![rule("docs", ext_condition(&[".pdf"]), "documents")],
        };

        assert!(matches!(
            set.validate(),
            Err(RuleError::InvalidExtension { .. })
        ));
    }

    #[test]
    fn rejects_destination_that_escapes_the_root() {
        let set = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![rule("escape", ext_condition(&["pdf"]), "../outside")],
        };

        assert!(matches!(
            set.validate(),
            Err(RuleError::InvalidTarget { .. })
        ));
    }

    #[test]
    fn rejects_absolute_destination() {
        let set = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![rule("abs", ext_condition(&["pdf"]), "C:\\Windows")],
        };

        assert!(matches!(
            set.validate(),
            Err(RuleError::InvalidTarget { .. })
        ));
    }

    #[test]
    fn rejects_unknown_fields_when_deserializing() {
        let json = r#"{"version":1,"rules":[{"id":"x","when":{"bogus":true},"then":{"move_to":"documents"}}]}"#;
        let parsed = serde_json::from_str::<RuleSet>(json);
        assert!(
            parsed.is_err(),
            "unknown condition field should be rejected"
        );
    }

    #[test]
    fn accepts_trash_action() {
        let set = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![Rule {
                id: "old-downloads".to_string(),
                priority: 0,
                when: ext_condition(&["tmp"]),
                then: Action {
                    move_to: None,
                    trash: true,
                    create_dir: None,
                },
            }],
        };

        assert!(set.validate().is_ok());
    }

    #[test]
    fn rejects_rule_with_multiple_actions() {
        let set = RuleSet {
            version: CURRENT_RULES_VERSION,
            rules: vec![Rule {
                id: "ambiguous".to_string(),
                priority: 0,
                when: ext_condition(&["tmp"]),
                then: Action {
                    move_to: Some("documents".to_string()),
                    trash: true,
                    create_dir: None,
                },
            }],
        };

        assert!(matches!(
            set.validate(),
            Err(RuleError::InvalidTarget { .. })
        ));
    }

    #[test]
    fn converts_contract_definition_with_all_conditions() {
        let set = RuleSet::from_contract_definition(
            "server-rule",
            7,
            json!({
                "match": "ALL",
                "conditions": [
                    {"field": "extension", "operator": "IN", "value": [".PDF"]},
                    {"field": "modifiedAgeDays", "operator": "GTE", "value": 30},
                    {"field": "createdAgeDays", "operator": "GT", "value": 3},
                    {"field": "sizeBytes", "operator": "LTE", "value": 1048576},
                    {"field": "relativePath", "operator": "STARTS_WITH", "value": "Downloads"},
                    {"field": "fileKind", "operator": "EQ", "value": "FILE"},
                    {"field": "name", "operator": "CONTAINS", "value": "report"}
                ],
                "action": {"type": "MOVE", "destinationTemplate": "Archive/PDF"}
            }),
        )
        .expect("convert contract definition");

        assert_eq!(set.rules.len(), 1);
        let rule = &set.rules[0];
        assert_eq!(rule.id, "server-rule");
        assert_eq!(rule.priority, 7);
        assert_eq!(
            rule.when.extension_in.as_deref(),
            Some(&["pdf".to_string()][..])
        );
        assert_eq!(rule.when.modified_age_days_gte, Some(30));
        assert_eq!(rule.when.created_age_days_gt, Some(3));
        assert_eq!(rule.when.size_bytes_lte, Some(1_048_576));
        assert_eq!(
            rule.when.relative_path_starts_with.as_deref(),
            Some("Downloads")
        );
        assert_eq!(rule.when.file_kind, Some(FileKind::File));
        assert_eq!(rule.when.name_contains.as_deref(), Some("report"));
        assert_eq!(rule.then.move_to.as_deref(), Some("Archive/PDF"));
    }

    #[test]
    fn converts_contract_any_definition_into_first_match_rules() {
        let set = RuleSet::from_contract_definition(
            "server-rule",
            0,
            json!({
                "match": "ANY",
                "conditions": [
                    {"field": "name", "operator": "ENDS_WITH", "value": ".tmp"},
                    {"field": "sizeBytes", "operator": "GTE", "value": 10}
                ],
                "action": {"type": "CREATE_DIR", "relativePath": "Archive"}
            }),
        )
        .expect("convert any definition");

        assert_eq!(set.rules.len(), 2);
        assert_eq!(set.rules[0].id, "server-rule-any-1");
        assert_eq!(set.rules[0].when.name_ends_with.as_deref(), Some(".tmp"));
        assert_eq!(set.rules[0].then.create_dir.as_deref(), Some("Archive"));
        assert_eq!(set.rules[1].id, "server-rule-any-2");
        assert_eq!(set.rules[1].when.size_bytes_gte, Some(10));
        assert_eq!(set.rules[1].then.create_dir.as_deref(), Some("Archive"));
    }
}
