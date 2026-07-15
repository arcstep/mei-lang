use super::types::*;

pub(super) fn access_assets() -> Vec<AssetSeed> {
    vec![
        AssetSeed {
            id: "meilang_access_skill",
            topic: "workflow",
            kind: "skill_entry",
            title: "MeiLang Access Skill",
            rel_path: "agent/guides/access-skills/SKILL.md",
            install_rel_path: "runtime/platform/skills/meilang-access/SKILL.md",
            summary: "Access-side entrypoint for standalone or host-bound AI tools.",
            injection_roles: &["system", "workflow"],
        },
        AssetSeed {
            id: "access_profile",
            topic: "profile",
            kind: "profile",
            title: "Access Profile",
            rel_path: "agent/guides/access-profile.md",
            install_rel_path: "runtime/platform/profiles/access.md",
            summary: "World-first access profile guidance for runtime-side tools.",
            injection_roles: &["system", "world"],
        },
        AssetSeed {
            id: "access_workflow",
            topic: "workflow",
            kind: "guide",
            title: "Access Workflow",
            rel_path: "agent/guides/access-skills/workflow.md",
            install_rel_path: "runtime/platform/skills/meilang-access/workflow.md",
            summary: "Companion workflow guide for query-state-aware access questions.",
            injection_roles: &["workflow", "world"],
        },
    ]
}
