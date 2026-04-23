use clap::{Parser, Subcommand};

use cascade_agent::agent::AgentLoop;
use cascade_agent::config::AgentConfig;

#[derive(Parser)]
#[command(name = "cascade-agent", version, about = "Async LLM agentic engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the agent with a prompt
    Run {
        /// The prompt to send to the agent
        prompt: String,
        /// Path to config file
        #[arg(long, default_value = "config.toml")]
        config: String,
    },
    /// Initialize a new config file
    Init {
        /// Output path for config file
        #[arg(long, default_value = "config.toml")]
        output: String,
    },
    /// List discovered skills
    Skills {
        #[arg(long, default_value = "config.toml")]
        config: String,
    },
    /// List all plans
    Plans {
        #[arg(long, default_value = "config.toml")]
        config: String,
    },
    /// Show a specific plan
    ShowPlan {
        /// Path to the plan file
        plan_path: String,
        #[arg(long, default_value = "config.toml")]
        config: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cascade_agent=info,tokio=info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run { prompt, config } => {
            let config = AgentConfig::load(std::path::Path::new(&config))?;
            let mut agent = AgentLoop::new(config).await?;
            let result = agent.run(prompt).await?;
            println!("{}", result);
        }
        Commands::Init { output } => {
            let default_config = include_str!("../config.example.toml");
            std::fs::write(&output, default_config)?;
            println!("Config file written to {}", output);
        }
        Commands::Skills { config } => {
            let config = AgentConfig::load(std::path::Path::new(&config))?;
            let mut skill_manager = cascade_agent::skills::SkillManager::new(
                std::path::PathBuf::from(&config.paths.skills_dir),
            )?;
            let discovered = skill_manager.discover()?;
            if discovered.is_empty() {
                println!("No skills discovered in {}", config.paths.skills_dir);
            } else {
                println!("Discovered {} skill(s):", discovered.len());
                for name in &discovered {
                    if let Some(skill) = skill_manager.get(name) {
                        println!(
                            "  - {} (v{})",
                            skill.metadata.name,
                            skill.metadata.version.as_deref().unwrap_or("?")
                        );
                        println!("    {}", skill.metadata.description);
                    }
                }
            }
        }
        Commands::Plans { config } => {
            let config = AgentConfig::load(std::path::Path::new(&config))?;
            let pm = cascade_agent::planning::PlanManager::new(std::path::PathBuf::from(
                &config.paths.plans_dir,
            ))?;
            let plans = pm.list_plans()?;
            if plans.is_empty() {
                println!("No plans found in {}", config.paths.plans_dir);
            } else {
                println!("Found {} plan(s):", plans.len());
                for path in &plans {
                    match pm.load_plan(path) {
                        Ok(plan) => {
                            let status_icon = match plan.status {
                                cascade_agent::planning::types::PlanStatus::Draft => "D",
                                cascade_agent::planning::types::PlanStatus::PendingApproval => "P",
                                cascade_agent::planning::types::PlanStatus::Approved => "A",
                                cascade_agent::planning::types::PlanStatus::InProgress => "I",
                                cascade_agent::planning::types::PlanStatus::Completed => "C",
                                cascade_agent::planning::types::PlanStatus::Cancelled => "X",
                            };
                            println!(
                                "  [{}] {} ({}/{} steps) — {}",
                                status_icon,
                                plan.title,
                                plan.steps
                                    .iter()
                                    .filter(|s| s.status
                                        == cascade_agent::planning::types::StepStatus::Completed)
                                    .count(),
                                plan.steps.len(),
                                path.display()
                            );
                        }
                        Err(e) => {
                            println!("  [?] Error loading {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }
        Commands::ShowPlan { plan_path, config } => {
            let config = AgentConfig::load(std::path::Path::new(&config))?;
            let pm = cascade_agent::planning::PlanManager::new(std::path::PathBuf::from(
                &config.paths.plans_dir,
            ))?;
            let plan_path = std::path::Path::new(&plan_path);
            if !plan_path.exists() {
                anyhow::bail!("Plan file not found: {}", plan_path.display());
            }
            let plan = pm.load_plan(plan_path)?;
            println!("{}", pm.render_markdown(&plan));
        }
    }

    Ok(())
}
