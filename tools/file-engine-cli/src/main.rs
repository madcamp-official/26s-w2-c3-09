mod analyzer;
mod path_guard;

use std::env;
use std::process;

use analyzer::analyze_root;
use path_guard::PathGuard;

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
        _ => print_usage_and_exit(),
    }
}

fn print_usage_and_exit() -> ! {
    eprintln!("usage:");
    eprintln!("  file-engine-cli guard <managed-root> <relative-path>");
    eprintln!("  file-engine-cli analyze <managed-root>");
    process::exit(2);
}
