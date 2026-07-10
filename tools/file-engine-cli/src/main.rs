mod analyzer;
mod execute;
mod journal;
mod path_guard;
mod precondition;
mod proposal;
mod rules;
mod undo;

use std::env;
use std::process;

use analyzer::analyze_root;
use execute::{execute_proposals, execute_root};
use journal::write_planned_journal;
use path_guard::PathGuard;
use precondition::{precheck_proposals, precheck_root};
use proposal::{propose_for_root, read_proposal_file};
use undo::undo_root;

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
            let proposal_path = parse_proposal_option(args);

            let result = match proposal_path {
                Some(path) => read_proposal_file(path)
                    .map_err(precondition::PrecheckError::Proposal)
                    .and_then(|proposal| precheck_proposals(root, proposal)),
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
            let proposal_path = parse_proposal_option(args);

            let result = match proposal_path {
                Some(path) => read_proposal_file(path)
                    .map_err(execute::ExecuteError::PrecheckProposal)
                    .and_then(|proposal| execute_proposals(root, proposal)),
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

fn parse_proposal_option(args: impl Iterator<Item = String>) -> Option<String> {
    let mut args = args;
    match args.next().as_deref() {
        Some("--proposal") => args.next(),
        Some(_) => print_usage_and_exit(),
        None => None,
    }
}

fn print_usage_and_exit() -> ! {
    eprintln!("usage:");
    eprintln!("  file-engine-cli guard <managed-root> <relative-path>");
    eprintln!("  file-engine-cli analyze <managed-root>");
    eprintln!("  file-engine-cli propose <managed-root>");
    eprintln!("  file-engine-cli precheck <managed-root> [--proposal <proposal.json>]");
    eprintln!("  file-engine-cli journal <managed-root>");
    eprintln!("  file-engine-cli execute <managed-root> [--proposal <proposal.json>]");
    eprintln!("  file-engine-cli undo <managed-root>");
    process::exit(2);
}
