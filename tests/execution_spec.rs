use savant_executioner::{execution::ExecutionSpec, savant::Task};

#[test]
fn parses_ready_workspace_config() {
    let task = Task {
        task_id: "task".into(),
        workspace_id: "ws".into(),
        title: "task".into(),
        description: "".into(),
        status: "todo".into(),
        priority: "medium".into(),
        depends_on: vec![],
        colosseum_ready: true,
        colosseum_config: serde_json::json!({"repository":"/tmp/repo","provider":"codex"}),
    };
    let parsed = ExecutionSpec::from_task(&task).unwrap();
    assert_eq!(parsed.provider, "codex");
}
