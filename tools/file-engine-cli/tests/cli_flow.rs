use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_file-engine-cli"))
}

fn run_success(args: &[&str]) -> String {
    let output = cli().args(args).output().expect("run cli");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("stdout utf8")
}

fn run_failure(args: &[&str]) -> String {
    let output = cli().args(args).output().expect("run cli");

    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stderr).expect("stderr utf8")
}

fn path_string(path: &Path) -> String {
    path.display().to_string()
}

#[test]
fn approve_execute_and_undo_round_trip() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().join("root");
    let inbox = root.join("inbox");
    fs::create_dir_all(&inbox).expect("create inbox");
    fs::write(inbox.join("note.md"), "# note").expect("write note");
    fs::write(inbox.join("photo.png"), "png").expect("write photo");

    let proposal_json = run_success(&["propose", &path_string(&root)]);
    let proposal_path = temp.path().join("proposal.json");
    fs::write(&proposal_path, &proposal_json).expect("write proposal");

    let proposal: Value = serde_json::from_str(&proposal_json).expect("parse proposal json");
    let note_proposal_id = proposal["proposals"]
        .as_array()
        .expect("proposal list")
        .iter()
        .find(|proposal| proposal["from"] == "inbox/note.md")
        .and_then(|proposal| proposal["proposal_id"].as_str())
        .expect("note proposal id");

    let decision_path = temp.path().join("decision.jsonl");
    fs::write(
        &decision_path,
        format!(r#"{{"proposal_id":"{note_proposal_id}","decision":"approved"}}"#),
    )
    .expect("write decision");

    run_success(&[
        "precheck",
        &path_string(&root),
        "--proposal",
        &path_string(&proposal_path),
        "--decision",
        &path_string(&decision_path),
    ]);

    run_success(&[
        "execute",
        &path_string(&root),
        "--proposal",
        &path_string(&proposal_path),
        "--decision",
        &path_string(&decision_path),
    ]);

    assert!(!inbox.join("note.md").exists());
    assert!(root.join("documents").join("note.md").exists());
    assert!(inbox.join("photo.png").exists());

    run_success(&["undo", &path_string(&root)]);

    assert!(inbox.join("note.md").exists());
    assert!(!root.join("documents").join("note.md").exists());
    assert!(inbox.join("photo.png").exists());
}

#[test]
fn cli_rejects_unknown_decision_id() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().join("root");
    let inbox = root.join("inbox");
    fs::create_dir_all(&inbox).expect("create inbox");
    fs::write(inbox.join("note.md"), "# note").expect("write note");

    let proposal_json = run_success(&["propose", &path_string(&root)]);
    let proposal_path = temp.path().join("proposal.json");
    fs::write(&proposal_path, proposal_json).expect("write proposal");

    let decision_path = temp.path().join("decision.jsonl");
    fs::write(
        &decision_path,
        r#"{"proposal_id":"move:missing.txt:documents/missing.txt","decision":"approved"}"#,
    )
    .expect("write decision");

    let stderr = run_failure(&[
        "precheck",
        &path_string(&root),
        "--proposal",
        &path_string(&proposal_path),
        "--decision",
        &path_string(&decision_path),
    ]);

    assert!(stderr.contains("unknown proposal_id"));
}

#[test]
fn execute_reports_rejected_decision() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().join("root");
    let inbox = root.join("inbox");
    fs::create_dir_all(&inbox).expect("create inbox");
    fs::write(inbox.join("note.md"), "# note").expect("write note");

    let proposal_json = run_success(&["propose", &path_string(&root)]);
    let proposal_path = temp.path().join("proposal.json");
    fs::write(&proposal_path, &proposal_json).expect("write proposal");

    let proposal: Value = serde_json::from_str(&proposal_json).expect("parse proposal json");
    let note_proposal_id = proposal["proposals"]
        .as_array()
        .expect("proposal list")
        .first()
        .and_then(|proposal| proposal["proposal_id"].as_str())
        .expect("note proposal id");

    let decision_path = temp.path().join("decision.jsonl");
    fs::write(
        &decision_path,
        format!(
            r#"{{"proposal_id":"{note_proposal_id}","decision":"rejected","reason":"keep in inbox"}}"#
        ),
    )
    .expect("write decision");

    let execute_json = run_success(&[
        "execute",
        &path_string(&root),
        "--proposal",
        &path_string(&proposal_path),
        "--decision",
        &path_string(&decision_path),
    ]);
    let report: Value = serde_json::from_str(&execute_json).expect("parse execute json");

    assert_eq!(report["executed_count"], 0);
    assert_eq!(report["rejected_count"], 1);
    assert_eq!(report["results"][0]["status"], "rejected");
    assert_eq!(report["results"][0]["reason"], "keep in inbox");
    assert!(inbox.join("note.md").exists());
    assert!(!root.join("documents").join("note.md").exists());
}
