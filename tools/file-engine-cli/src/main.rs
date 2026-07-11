use std::env;
use std::process;

use file_engine_cli::analyzer::{self, analyze_root};
use file_engine_cli::browse::{self, browse_root};
use file_engine_cli::decision::{self, apply_decisions, read_decision_file, DecisionApplication};
use file_engine_cli::execute::{self, execute_decision_application, execute_root};
use file_engine_cli::file_index::{list_index, reindex_root, search_index};
use file_engine_cli::file_ops::{self, create_empty_file, rename_file};
use file_engine_cli::journal::{self, recover_journal, write_planned_journal};
use file_engine_cli::path_guard::PathGuard;
use file_engine_cli::precondition::{self, precheck_proposals, precheck_root};
use file_engine_cli::proposal::{self, propose_for_root, read_proposal_file};
use file_engine_cli::rules::{self, load_rule_set_for_root};
use file_engine_cli::trash::{self, trash_file};
use file_engine_cli::undo::{self, undo_root};

fn main() {
    let mut args = env::args().skip(1);

    match args.next().as_deref() {
        Some("guard") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };
            let Some(relative_path) = args.next() else {
                print_usage_and_exit();
            };

            match PathGuard::new(root).and_then(|guard| guard.resolve_existing(relative_path)) {
                Ok(path) => println!("{}", path.display()),
                Err(error) => {
                    eprintln!("path rejected: {error}");
                    process::exit(1);
                }
            }
        }
        Some("analyze") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };

            match analyze_root(root).and_then(|report| {
                serde_json::to_string_pretty(&report)
                    .map_err(|error| analyzer::AnalyzeError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("analyze failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("browse") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };
            let path = parse_path_option(args);

            match browse_root(root, path.as_deref()).and_then(|report| {
                serde_json::to_string_pretty(&report)
                    .map_err(|error| browse::BrowseError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("browse failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("index") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };

            match reindex_root(root) {
                Ok(report) => print_json_or_exit(&report),
                Err(error) => {
                    eprintln!("index failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("search") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };
            let Some(query) = args.next() else {
                print_usage_and_exit();
            };

            let result = if query.is_empty() {
                list_index(root)
            } else {
                search_index(root, &query)
            };

            match result {
                Ok(report) => print_json_or_exit(&report),
                Err(error) => {
                    eprintln!("search failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("rules") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };

            match load_rule_set_for_root(root).and_then(|rule_set| {
                serde_json::to_string_pretty(&rule_set)
                    .map_err(|error| rules::RuleError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("rules failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("propose") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };

            match propose_for_root(root).and_then(|report| {
                serde_json::to_string_pretty(&report)
                    .map_err(|error| proposal::ProposalError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("proposal failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("precheck") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };
            let options = parse_options(args);

            let result = match options.proposal {
                Some(path) => read_proposal_file(path)
                    .map_err(precondition::PrecheckError::Proposal)
                    .and_then(|proposal| {
                        apply_optional_decisions(proposal, options.decision)
                            .map_err(precondition::PrecheckError::Decision)
                    })
                    .and_then(|application| precheck_proposals(root, application.approved)),
                None => precheck_root(root),
            };

            match result.and_then(|report| {
                serde_json::to_string_pretty(&report)
                    .map_err(|error| precondition::PrecheckError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("precheck failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("journal") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };

            match write_planned_journal(root).and_then(|report| {
                serde_json::to_string_pretty(&report)
                    .map_err(|error| journal::JournalError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("journal failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("recover-journal") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };

            match recover_journal(root).and_then(|report| {
                serde_json::to_string_pretty(&report)
                    .map_err(|error| journal::JournalError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("recover-journal failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("execute") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };
            let options = parse_options(args);

            let result = match options.proposal {
                Some(path) => read_proposal_file(path)
                    .map_err(execute::ExecuteError::PrecheckProposal)
                    .and_then(|proposal| {
                        apply_optional_decisions(proposal, options.decision)
                            .map_err(execute::ExecuteError::Decision)
                    })
                    .and_then(|application| execute_decision_application(root, application)),
                None => execute_root(root),
            };

            match result.and_then(|report| {
                serde_json::to_string_pretty(&report)
                    .map_err(|error| execute::ExecuteError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("execute failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("trash") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };
            let Some(relative_path) = args.next() else {
                print_usage_and_exit();
            };

            match trash_file(root, relative_path).and_then(|report| {
                serde_json::to_string_pretty(&report)
                    .map_err(|error| trash::TrashError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("trash failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("create-file") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };
            let Some(relative_path) = args.next() else {
                print_usage_and_exit();
            };

            match create_empty_file(root, relative_path).and_then(|report| {
                serde_json::to_string_pretty(&report)
                    .map_err(|error| file_ops::FileOpError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("create-file failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("rename-file") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };
            let Some(relative_path) = args.next() else {
                print_usage_and_exit();
            };
            let Some(new_name) = args.next() else {
                print_usage_and_exit();
            };

            match rename_file(root, relative_path, new_name).and_then(|report| {
                serde_json::to_string_pretty(&report)
                    .map_err(|error| file_ops::FileOpError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("rename-file failed: {error}");
                    process::exit(1);
                }
            }
        }
        Some("undo") => {
            let Some(root) = args.next() else {
                print_usage_and_exit();
            };

            match undo_root(root).and_then(|report| {
                serde_json::to_string_pretty(&report)
                    .map_err(|error| undo::UndoError::Serialize(error.to_string()))
            }) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("undo failed: {error}");
                    process::exit(1);
                }
            }
        }
        _ => print_usage_and_exit(),
    }
}

fn print_json_or_exit<T: serde::Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("cannot serialize report: {error}");
            process::exit(1);
        }
    }
}

fn apply_optional_decisions(
    proposal: proposal::ProposalReport,
    decision_path: Option<String>,
) -> Result<DecisionApplication, decision::DecisionError> {
    match decision_path {
        Some(path) => {
            let decisions = read_decision_file(path)?;
            apply_decisions(proposal, &decisions)
        }
        None => Ok(DecisionApplication {
            approved: proposal,
            rejected: Vec::new(),
        }),
    }
}

fn parse_path_option(args: impl Iterator<Item = String>) -> Option<String> {
    let mut args = args;
    let mut path = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--path" => path = args.next(),
            _ => print_usage_and_exit(),
        }
    }

    path
}

#[derive(Debug, Default)]
struct CommandOptions {
    proposal: Option<String>,
    decision: Option<String>,
}

fn parse_options(args: impl Iterator<Item = String>) -> CommandOptions {
    let mut options = CommandOptions::default();
    let mut args = args;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--proposal" => options.proposal = args.next(),
            "--decision" => options.decision = args.next(),
            _ => print_usage_and_exit(),
        }
    }

    if options.decision.is_some() && options.proposal.is_none() {
        print_usage_and_exit();
    }

    options
}

fn print_usage_and_exit() -> ! {
    eprintln!("usage:");
    eprintln!("  file-engine-cli guard <managed-root> <relative-path>");
    eprintln!("  file-engine-cli analyze <managed-root>");
    eprintln!("  file-engine-cli browse <managed-root> [--path <relative-path>]");
    eprintln!("  file-engine-cli index <managed-root>");
    eprintln!("  file-engine-cli search <managed-root> <query>");
    eprintln!("  file-engine-cli rules <managed-root>");
    eprintln!("  file-engine-cli propose <managed-root>");
    eprintln!(
        "  file-engine-cli precheck <managed-root> [--proposal <proposal.json> [--decision <decision.jsonl>]]"
    );
    eprintln!("  file-engine-cli journal <managed-root>");
    eprintln!("  file-engine-cli recover-journal <managed-root>");
    eprintln!(
        "  file-engine-cli execute <managed-root> [--proposal <proposal.json> [--decision <decision.jsonl>]]"
    );
    eprintln!("  file-engine-cli trash <managed-root> <relative-path>");
    eprintln!("  file-engine-cli create-file <managed-root> <relative-path>");
    eprintln!("  file-engine-cli rename-file <managed-root> <relative-path> <new-file-name>");
    eprintln!("  file-engine-cli undo <managed-root>");
    process::exit(2);
}
