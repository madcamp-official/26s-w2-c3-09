use std::env;
use std::process;

use file_engine_cli::analyzer::{self, analyze_root};
use file_engine_cli::decision::{self, apply_decisions, read_decision_file, DecisionApplication};
use file_engine_cli::execute::{self, execute_decision_application, execute_root};
use file_engine_cli::journal::{self, write_planned_journal};
use file_engine_cli::path_guard::PathGuard;
use file_engine_cli::precondition::{self, precheck_proposals, precheck_root};
use file_engine_cli::proposal::{self, propose_for_root, read_proposal_file};
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
    eprintln!("  file-engine-cli propose <managed-root>");
    eprintln!(
        "  file-engine-cli precheck <managed-root> [--proposal <proposal.json> [--decision <decision.jsonl>]]"
    );
    eprintln!("  file-engine-cli journal <managed-root>");
    eprintln!(
        "  file-engine-cli execute <managed-root> [--proposal <proposal.json> [--decision <decision.jsonl>]]"
    );
    eprintln!("  file-engine-cli undo <managed-root>");
    process::exit(2);
}
