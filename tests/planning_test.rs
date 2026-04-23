use cascade_agent::planning::{
    types::{PlanStatus, StepStatus},
    PlanManager,
};
use tempfile::tempdir;

#[test]
fn plan_create_and_load_roundtrip() {
    let dir = tempdir().unwrap();
    let pm = PlanManager::new(dir.path().to_path_buf()).unwrap();

    let plan = pm
        .create_plan(
            "task-123",
            "Test Plan",
            vec!["Step one".into(), "Step two".into(), "Step three".into()],
        )
        .unwrap();

    assert_eq!(plan.title, "Test Plan");
    assert_eq!(plan.task_id, "task-123");
    assert_eq!(plan.steps.len(), 3);
    assert_eq!(plan.status, PlanStatus::Draft);
    assert!(plan.file_path.exists());

    let loaded = pm.load_plan(&plan.file_path).unwrap();
    assert_eq!(loaded.id, plan.id);
    assert_eq!(loaded.task_id, plan.task_id);
    assert_eq!(loaded.title, plan.title);
    assert_eq!(loaded.status, PlanStatus::Draft);
    assert_eq!(loaded.steps.len(), 3);
}

#[test]
fn plan_update_step_preserves_on_reload() {
    let dir = tempdir().unwrap();
    let pm = PlanManager::new(dir.path().to_path_buf()).unwrap();

    let mut plan = pm
        .create_plan("task-456", "Update Test", vec!["Do thing".into()])
        .unwrap();

    pm.update_step(&mut plan, 1, StepStatus::Completed, Some("Done!"))
        .unwrap();

    let loaded = pm.load_plan(&plan.file_path).unwrap();
    assert_eq!(loaded.steps[0].status, StepStatus::Completed);
    assert_eq!(loaded.steps[0].result.as_deref(), Some("Done!"));
    assert_eq!(loaded.status, PlanStatus::Completed);
}

#[test]
fn plan_mark_step_failed_preserves_status() {
    let dir = tempdir().unwrap();
    let pm = PlanManager::new(dir.path().to_path_buf()).unwrap();

    let mut plan = pm
        .create_plan(
            "task-789",
            "Fail Test",
            vec!["Risky step".into(), "Safe step".into()],
        )
        .unwrap();

    pm.update_step(&mut plan, 1, StepStatus::Failed, Some("Error occurred"))
        .unwrap();

    let loaded = pm.load_plan(&plan.file_path).unwrap();
    assert_eq!(loaded.steps[0].status, StepStatus::Failed);
    assert_eq!(loaded.steps[0].result.as_deref(), Some("Error occurred"));
    assert_eq!(loaded.status, PlanStatus::Draft);
}

#[test]
fn plan_list_plans() {
    let dir = tempdir().unwrap();
    let pm = PlanManager::new(dir.path().to_path_buf()).unwrap();

    pm.create_plan("t1", "Plan A", vec!["Step".into()]).unwrap();
    pm.create_plan("t2", "Plan B", vec!["Step".into()]).unwrap();

    let plans = pm.list_plans().unwrap();
    assert_eq!(plans.len(), 2);
}

#[test]
fn plan_render_markdown_contains_metadata() {
    let dir = tempdir().unwrap();
    let pm = PlanManager::new(dir.path().to_path_buf()).unwrap();

    let plan = pm
        .create_plan("task-meta", "Meta Test", vec!["Step".into()])
        .unwrap();

    let rendered = pm.render_markdown(&plan);
    assert!(rendered.contains(&format!("<!-- plan_id: {} -->", plan.id)));
    assert!(rendered.contains("<!-- task_id: task-meta -->"));
    assert!(rendered.contains("<!-- plan_status: Draft -->"));
    // step_status comment now appears before the step line
    let lines: Vec<&str> = rendered.lines().collect();
    let mut found_step_line = false;
    let mut found_status_before = false;
    for i in 0..lines.len() {
        if lines[i].contains("<!-- step_status_1:") {
            // Check that the step line comes after this comment
            for j in (i + 1)..lines.len() {
                if lines[j].contains("**Step 1:**") {
                    found_status_before = true;
                    break;
                }
            }
        }
        if lines[i].contains("**Step 1:**") {
            found_step_line = true;
        }
    }
    assert!(found_step_line, "Step line not found in rendered markdown");
    assert!(
        found_status_before,
        "step_status comment should appear before the step line"
    );
}
