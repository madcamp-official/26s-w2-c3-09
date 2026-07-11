use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub older_than_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_matches: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Action {
    /// Destination directory relative to the managed root. Validated to stay inside the root
    /// (no absolute paths, no `..`) because this value can come from an untrusted source.
    pub move_to: String,
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

        validate_relative_dir(&self.then.move_to).map_err(|message| RuleError::InvalidTarget {
            rule: self.id.clone(),
            target: self.then.move_to.clone(),
            message,
        })?;

        Ok(())
    }
}

impl Condition {
    fn is_empty(&self) -> bool {
        self.extension_in.is_none() && self.older_than_days.is_none() && self.name_matches.is_none()
    }
}

/// Rejects a `move_to` that is empty, absolute, or escapes the managed root. This mirrors the
/// component rules `PathGuard` enforces, but runs at rule-definition time so a bad rule is
/// refused before it ever produces a proposal.
fn validate_relative_dir(target: &str) -> Result<(), String> {
    if target.trim().is_empty() {
        return Err("destination is empty".to_string());
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
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    Ok(())
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
    use super::{Action, Condition, Rule, RuleError, RuleSet, CURRENT_RULES_VERSION};

    fn rule(id: &str, when: Condition, move_to: &str) -> Rule {
        Rule {
            id: id.to_string(),
            priority: 0,
            when,
            then: Action {
                move_to: move_to.to_string(),
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
}
