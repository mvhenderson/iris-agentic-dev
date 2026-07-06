//! Unit tests for loading the ported jira_bugs task suite (T015). No live IRIS.

use iris_agentic_dev_core::benchmark::{load_embedded_tasks, load_tasks};
use std::path::PathBuf;

fn tasks_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/benchmark/tasks/jira_bugs")
}

#[test]
fn loads_exactly_22_primary_suite_tasks() {
    let tasks = load_tasks(&tasks_dir()).expect("tasks should load");
    assert_eq!(tasks.len(), 22);
}

#[test]
fn every_task_has_non_empty_required_fields() {
    let tasks = load_tasks(&tasks_dir()).expect("tasks should load");
    for task in &tasks {
        assert!(!task.task_id.is_empty(), "task_id must not be empty");
        assert!(
            !task.initial_code.files.is_empty(),
            "initial_code.files must not be empty for {}",
            task.task_id
        );
        assert!(
            !task.test_code.content.is_empty(),
            "test_code.content must not be empty for {}",
            task.task_id
        );
    }
}

#[test]
fn loading_nonexistent_dir_errors_rather_than_panics() {
    let result = load_tasks(&PathBuf::from("/nonexistent/path/that/does/not/exist"));
    assert!(result.is_err());
}

// ── load_embedded_tasks — the CLI's actual runtime path ─────────────────────
//
// Regression: the CLI benchmark command used to call load_tasks() with a directory
// built from env!("CARGO_MANIFEST_DIR") — a compile-time constant that bakes in the
// BUILD machine's path (e.g. /home/runner/work/... on a GitHub Actions cross-compile
// runner). That path doesn't exist on the machine actually running the shipped
// binary, so `benchmark --skill ...` failed for every user with "failed to read task
// directory ... os error 3/2" regardless of platform. The fix: embed the task JSON
// at compile time via include_str! (load_embedded_tasks) so there is no runtime
// filesystem dependency at all. These tests exercise ONLY load_embedded_tasks() —
// they must pass even if the crate is copied to a machine with no source tree,
// which is exactly what the CARGO_MANIFEST_DIR-based tests above can never catch
// (they always run from a checked-out source tree during `cargo test`).

#[test]
fn embedded_tasks_load_without_any_filesystem_path() {
    let tasks = load_embedded_tasks().expect("embedded tasks should load with no fs access");
    assert_eq!(tasks.len(), 22);
}

#[test]
fn embedded_tasks_match_directory_tasks_exactly() {
    let from_dir = load_tasks(&tasks_dir()).expect("dir tasks should load");
    let from_embed = load_embedded_tasks().expect("embedded tasks should load");
    assert_eq!(from_dir.len(), from_embed.len());
    let dir_ids: Vec<_> = from_dir.iter().map(|t| t.task_id.clone()).collect();
    let embed_ids: Vec<_> = from_embed.iter().map(|t| t.task_id.clone()).collect();
    assert_eq!(
        dir_ids, embed_ids,
        "embedded task list has drifted from the jira_bugs/ directory — a .json file \
         was added/removed/renamed on disk without updating the EMBEDDED_TASKS list \
         in benchmark/mod.rs"
    );
}
