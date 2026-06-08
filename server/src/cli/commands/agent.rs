use anyhow::Result;

use crate::agent_runtime;
use super::super::args::{AgentCommand, AgentRuntimeArgs, AgentSkillArgs, AgentSkillCommand};
use super::super::util::resolve_package_root;

pub fn agent_command(args: AgentRuntimeArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    match args.command {
        AgentCommand::Skill(skill_args) => {
            let AgentSkillArgs {
                source_root,
                command,
            } = skill_args;
            let source_root = if source_root.is_absolute() {
                source_root
            } else {
                package_root.join(source_root)
            };
            match command {
                AgentSkillCommand::Status => {
                    let status = agent_runtime::runtime::managed_agent_skill_status_for_root(
                        &package_root,
                        &source_root,
                    );
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
                AgentSkillCommand::Sync => {
                    let status = agent_runtime::runtime::sync_managed_agent_skill_for_root(
                        &package_root,
                        &source_root,
                    )?;
                    println!("{}", serde_json::to_string_pretty(&status)?);
                }
            }
        }
    }
    Ok(())
}
