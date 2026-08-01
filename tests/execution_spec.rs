use savant_executioner::{
    execution::{ExecutionPhase, ExecutionSpec, WorkType},
    savant::Task,
};

#[test]
fn parses_ready_workspace_config() {
    let task = Task {
        task_id: "task".into(),
        workspace_id: "ws".into(),
        title: "task".into(),
        description: "".into(),
        status: "todo".into(),
        colosseum_claimed_from: Some("ready".into()),
        priority: "medium".into(),
        depends_on: vec![],
        colosseum_ready: true,
        colosseum_config: serde_json::json!({"repository":"/tmp/repo","provider":"codex"}),
        comments: serde_json::json!([]),
    };
    let parsed = ExecutionSpec::from_task(&task).unwrap();
    assert_eq!(parsed.provider, "codex");
    assert_eq!(parsed.work_type, WorkType::Development);
    assert_eq!(
        ExecutionPhase::from_task(&task).unwrap(),
        ExecutionPhase::Work
    );
}

#[test]
fn maps_each_claimed_queue_to_an_execution_phase() {
    let mut task = Task {
        task_id: "task".into(),
        workspace_id: "ws".into(),
        title: "task".into(),
        description: "".into(),
        status: "in-progress".into(),
        colosseum_claimed_from: Some("grooming".into()),
        priority: "medium".into(),
        depends_on: vec![],
        colosseum_ready: false,
        colosseum_config: serde_json::json!({"work_type":"research","provider":"codex"}),
        comments: serde_json::json!([]),
    };
    assert_eq!(
        ExecutionPhase::from_task(&task).unwrap(),
        ExecutionPhase::Grooming
    );
    task.colosseum_claimed_from = Some("review".into());
    assert_eq!(
        ExecutionPhase::from_task(&task).unwrap(),
        ExecutionPhase::Review
    );
    task.colosseum_claimed_from = Some("approved".into());
    assert_eq!(
        ExecutionPhase::from_task(&task).unwrap(),
        ExecutionPhase::Merge
    );
}
