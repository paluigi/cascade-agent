//! Planning tools: create_plan, update_plan_step, list_plans, get_plan.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Tool, ToolResult};
use crate::planning::types::StepStatus;
use crate::planning::PlanManager;

// ---------------------------------------------------------------------------
// CreatePlanTool
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct CreatePlanTool {
    plan_manager: Arc<std::sync::Mutex<PlanManager>>,
    task_id: String,
}

impl CreatePlanTool {
    pub fn new(plan_manager: Arc<std::sync::Mutex<PlanManager>>, task_id: String) -> Self {
        Self {
            plan_manager,
            task_id,
        }
    }
}

#[async_trait]
impl Tool for CreatePlanTool {
    fn name(&self) -> &str {
        "create_plan"
    }

    fn description(&self) -> &str {
        "Create a new execution plan with a list of steps. Returns the plan ID and rendered markdown."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "The plan title."
                },
                "steps": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Ordered list of step descriptions."
                }
            },
            "required": ["title", "steps"]
        })
    }

    async fn execute(&self, args: Value) -> ToolResult {
        let title = match args.get("title").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return ToolResult::err("Missing required parameter 'title'"),
        };

        let steps: Vec<String> = match args.get("steps").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            None => {
                return ToolResult::err("Missing required parameter 'steps' (must be an array)")
            }
        };

        if steps.is_empty() {
            return ToolResult::err("Plan must have at least one step");
        }

        let pm = self.plan_manager.lock().unwrap();
        match pm.create_plan(&self.task_id, title, steps) {
            Ok(plan) => {
                let rendered = pm.render_markdown(&plan);
                ToolResult::ok(json!({
                    "plan_id": plan.id,
                    "title": plan.title,
                    "step_count": plan.steps.len(),
                    "file_path": plan.file_path.to_string_lossy(),
                    "rendered": rendered,
                }))
            }
            Err(e) => ToolResult::err(format!("Failed to create plan: {}", e)),
        }
    }
}

// ---------------------------------------------------------------------------
// UpdatePlanStepTool
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct UpdatePlanStepTool {
    plan_manager: Arc<std::sync::Mutex<PlanManager>>,
}

impl UpdatePlanStepTool {
    pub fn new(plan_manager: Arc<std::sync::Mutex<PlanManager>>) -> Self {
        Self { plan_manager }
    }
}

#[async_trait]
impl Tool for UpdatePlanStepTool {
    fn name(&self) -> &str {
        "update_plan_step"
    }

    fn description(&self) -> &str {
        "Update the status of a step in an existing plan. Status can be: in_progress, completed, failed, skipped."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "plan_file_path": {
                    "type": "string",
                    "description": "The file path of the plan to update."
                },
                "step_number": {
                    "type": "integer",
                    "description": "The step number (1-indexed)."
                },
                "status": {
                    "type": "string",
                    "enum": ["in_progress", "completed", "failed", "skipped"],
                    "description": "The new status for the step."
                },
                "result": {
                    "type": "string",
                    "description": "Optional result description for completed/failed steps."
                }
            },
            "required": ["plan_file_path", "step_number", "status"]
        })
    }

    async fn execute(&self, args: Value) -> ToolResult {
        let plan_file_path = match args.get("plan_file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter 'plan_file_path'"),
        };

        let step_number = match args.get("step_number").and_then(|v| v.as_u64()) {
            Some(n) => n as usize,
            None => return ToolResult::err("Missing required parameter 'step_number'"),
        };

        let status_str = match args.get("status").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::err("Missing required parameter 'status'"),
        };

        let status = match status_str {
            "in_progress" => StepStatus::InProgress,
            "completed" => StepStatus::Completed,
            "failed" => StepStatus::Failed,
            "skipped" => StepStatus::Skipped,
            _ => {
                return ToolResult::err(format!(
                    "Unknown status: '{}'. Use: in_progress, completed, failed, skipped",
                    status_str
                ))
            }
        };

        let result = args.get("result").and_then(|v| v.as_str());

        let pm = self.plan_manager.lock().unwrap();
        let path = std::path::Path::new(plan_file_path);

        let mut plan = match pm.load_plan(path) {
            Ok(p) => p,
            Err(e) => return ToolResult::err(format!("Failed to load plan: {}", e)),
        };

        match pm.update_step(&mut plan, step_number, status, result) {
            Ok(()) => {
                let rendered = pm.render_markdown(&plan);
                ToolResult::ok(json!({
                    "plan_id": plan.id,
                    "step_number": step_number,
                    "status": status_str,
                    "plan_status": format!("{:?}", plan.status),
                    "rendered": rendered,
                }))
            }
            Err(e) => ToolResult::err(format!("Failed to update step: {}", e)),
        }
    }
}

// ---------------------------------------------------------------------------
// ListPlansTool
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ListPlansTool {
    plan_manager: Arc<std::sync::Mutex<PlanManager>>,
}

impl ListPlansTool {
    pub fn new(plan_manager: Arc<std::sync::Mutex<PlanManager>>) -> Self {
        Self { plan_manager }
    }
}

#[async_trait]
impl Tool for ListPlansTool {
    fn name(&self) -> &str {
        "list_plans"
    }

    fn description(&self) -> &str {
        "List all existing plans in the plans directory."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: Value) -> ToolResult {
        let pm = self.plan_manager.lock().unwrap();
        match pm.list_plans() {
            Ok(paths) => {
                let plans: Vec<Value> = paths
                    .iter()
                    .filter_map(|p| {
                        let plan = pm.load_plan(p).ok()?;
                        Some(json!({
                            "plan_id": plan.id,
                            "title": plan.title,
                            "status": format!("{:?}", plan.status),
                            "steps": plan.steps.len(),
                            "file_path": p.to_string_lossy(),
                        }))
                    })
                    .collect();

                ToolResult::ok(json!({
                    "count": plans.len(),
                    "plans": plans,
                }))
            }
            Err(e) => ToolResult::err(format!("Failed to list plans: {}", e)),
        }
    }
}

// ---------------------------------------------------------------------------
// GetPlanTool
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct GetPlanTool {
    plan_manager: Arc<std::sync::Mutex<PlanManager>>,
}

impl GetPlanTool {
    pub fn new(plan_manager: Arc<std::sync::Mutex<PlanManager>>) -> Self {
        Self { plan_manager }
    }
}

#[async_trait]
impl Tool for GetPlanTool {
    fn name(&self) -> &str {
        "get_plan"
    }

    fn description(&self) -> &str {
        "Load and render a specific plan by its file path."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "plan_file_path": {
                    "type": "string",
                    "description": "The file path of the plan to load."
                }
            },
            "required": ["plan_file_path"]
        })
    }

    async fn execute(&self, args: Value) -> ToolResult {
        let plan_file_path = match args.get("plan_file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter 'plan_file_path'"),
        };

        let pm = self.plan_manager.lock().unwrap();
        match pm.load_plan(std::path::Path::new(plan_file_path)) {
            Ok(plan) => {
                let rendered = pm.render_markdown(&plan);
                ToolResult::ok(json!({
                    "plan_id": plan.id,
                    "title": plan.title,
                    "status": format!("{:?}", plan.status),
                    "task_id": plan.task_id,
                    "steps": plan.steps.iter().map(|s| json!({
                        "number": s.number,
                        "description": s.description,
                        "status": format!("{:?}", s.status),
                        "result": s.result,
                    })).collect::<Vec<_>>(),
                    "rendered": rendered,
                }))
            }
            Err(e) => ToolResult::err(format!("Failed to load plan: {}", e)),
        }
    }
}
