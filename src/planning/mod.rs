pub mod types;

use crate::error::Result;
use std::path::{Path, PathBuf};
use types::{Plan, PlanData, PlanStatus, StepStatus};

#[derive(Debug)]
pub struct PlanManager {
    plans_dir: PathBuf,
}

impl PlanManager {
    pub fn new(plans_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&plans_dir)?;
        Ok(Self { plans_dir })
    }

    pub fn create_plan(&self, task_id: &str, title: &str, steps: Vec<String>) -> Result<Plan> {
        let mut plan = Plan::new(task_id, title, steps);
        let filename = format!(
            "{}_{}.toml",
            plan.created_at.format("%Y%m%d_%H%M%S"),
            &plan.id[..8]
        );
        plan.file_path = self.plans_dir.join(filename);
        self.save_plan(&plan)?;
        Ok(plan)
    }

    pub fn save_plan(&self, plan: &Plan) -> Result<()> {
        let data = PlanData::from_plan(plan);
        let content = toml::to_string_pretty(&data)?;
        std::fs::write(&plan.file_path, content)?;
        Ok(())
    }

    pub fn load_plan(&self, file_path: &Path) -> Result<Plan> {
        let content = std::fs::read_to_string(file_path)?;
        let data: PlanData = toml::from_str(&content)?;
        data.to_plan(file_path)
    }

    pub fn list_plans(&self) -> Result<Vec<PathBuf>> {
        let mut plans: Vec<PathBuf> = std::fs::read_dir(&self.plans_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        plans.sort();
        Ok(plans)
    }

    pub fn update_step(
        &self,
        plan: &mut Plan,
        step_num: usize,
        status: StepStatus,
        result: Option<&str>,
    ) -> Result<()> {
        match status {
            StepStatus::Completed => {
                if !plan.mark_step_completed(step_num, result.map(String::from)) {
                    return Err(crate::error::AgentError::ConfigError(format!(
                        "Step {} not found in plan",
                        step_num
                    )));
                }
            }
            StepStatus::Failed => {
                if !plan.mark_step_failed(step_num, result.unwrap_or_default().to_string()) {
                    return Err(crate::error::AgentError::ConfigError(format!(
                        "Step {} not found in plan",
                        step_num
                    )));
                }
            }
            StepStatus::InProgress => {
                if !plan.mark_step_in_progress(step_num) {
                    return Err(crate::error::AgentError::ConfigError(format!(
                        "Step {} not found in plan",
                        step_num
                    )));
                }
            }
            _ => {
                if let Some(step) = plan.steps.iter_mut().find(|s| s.number == step_num) {
                    step.status = status;
                    step.result = result.map(String::from);
                    plan.updated_at = chrono::Utc::now();
                } else {
                    return Err(crate::error::AgentError::ConfigError(format!(
                        "Step {} not found in plan",
                        step_num
                    )));
                }
            }
        }
        self.save_plan(plan)?;
        Ok(())
    }

    pub fn render_markdown(&self, plan: &Plan) -> String {
        let status_emoji = match &plan.status {
            PlanStatus::Draft => "📝",
            PlanStatus::PendingApproval => "⏳",
            PlanStatus::Approved => "✅",
            PlanStatus::InProgress => "🔄",
            PlanStatus::Completed => "🏁",
            PlanStatus::Cancelled => "❌",
        };

        let step_emoji = |s: &StepStatus| match s {
            StepStatus::Pending => "⬜",
            StepStatus::InProgress => "🔄",
            StepStatus::Completed => "✅",
            StepStatus::Failed => "❌",
            StepStatus::Skipped => "⏭️",
        };

        let mut md = format!("# {}\n\n", plan.title);

        md.push_str(&format!("<!-- plan_id: {} -->\n", plan.id));
        md.push_str(&format!("<!-- task_id: {} -->\n", plan.task_id));
        md.push_str(&format!("<!-- plan_status: {:?} -->\n", plan.status));
        md.push_str(&format!(
            "<!-- created_at: {} -->\n",
            plan.created_at.to_rfc3339()
        ));
        md.push_str(&format!(
            "<!-- updated_at: {} -->\n",
            plan.updated_at.to_rfc3339()
        ));

        md.push_str(&format!("**Status:** {} {:?}\n", status_emoji, plan.status));
        md.push_str(&format!("**Task ID:** {}\n", plan.task_id));
        md.push_str(&format!("**Plan ID:** {}\n", plan.id));
        md.push_str(&format!(
            "**Created:** {}\n\n",
            plan.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        md.push_str("---\n\n## Steps\n\n");

        for step in &plan.steps {
            md.push_str(&format!(
                "<!-- step_status_{}: {:?} -->\n",
                step.number, step.status
            ));
            md.push_str(&format!(
                "{} **Step {}:** {}\n",
                step_emoji(&step.status),
                step.number,
                step.description
            ));
            if let Some(result) = &step.result {
                md.push_str(&format!("   - Result: {}\n", result));
            }
            md.push('\n');
        }

        md
    }
}
