use std::collections::BTreeMap;

use super::scope::CompileScope;

/// Target-driven MCG planning: collapse manifest scopes onto unique compile targets first.
#[derive(Debug, Clone)]
pub(crate) struct McgTargetPlan {
    pub unique_targets: Vec<String>,
    pub scopes_by_target: BTreeMap<String, Vec<CompileScope>>,
}

pub(crate) fn compile_target_key(scope: &CompileScope) -> String {
    scope
        .canonicalized()
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "__default_scope__".to_string())
}

pub(crate) fn build_mcg_target_plan(scopes: &[CompileScope]) -> McgTargetPlan {
    let mut scopes_by_target: BTreeMap<String, Vec<CompileScope>> = BTreeMap::new();
    for scope in scopes {
        scopes_by_target
            .entry(compile_target_key(scope))
            .or_default()
            .push(scope.clone());
    }
    let unique_targets = scopes_by_target.keys().cloned().collect();
    McgTargetPlan {
        unique_targets,
        scopes_by_target,
    }
}

/// Order scopes so each unique target is compiled once before alias scopes fan out.
pub(crate) fn order_scopes_target_first(scopes: Vec<CompileScope>) -> Vec<CompileScope> {
    let plan = build_mcg_target_plan(scopes.as_slice());
    let mut ordered = Vec::with_capacity(scopes.len());
    let mut seen = std::collections::BTreeSet::new();
    for target in plan.unique_targets {
        let Some(group) = plan.scopes_by_target.get(&target) else {
            continue;
        };
        for scope in group {
            if seen.insert(scope.key()) {
                ordered.push(scope.clone());
            }
        }
    }
    for scope in scopes {
        if seen.insert(scope.key()) {
            ordered.push(scope);
        }
    }
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_scopes_target_first_keeps_unique_targets() {
        let scopes = vec![
            CompileScope {
                requested_scene_id: Some("home".to_string()),
                requested_target_file: Some("src/scenes/home.mei".to_string()),
            },
            CompileScope {
                requested_scene_id: Some("drill".to_string()),
                requested_target_file: Some("src/scenes/home.mei".to_string()),
            },
            CompileScope {
                requested_scene_id: Some("detail".to_string()),
                requested_target_file: Some("src/scenes/detail-board.mei".to_string()),
            },
        ];
        let ordered = order_scopes_target_first(scopes);
        assert_eq!(ordered.len(), 3);
        assert_eq!(
            compile_target_key(&ordered[0]),
            "src/scenes/home.mei".to_string()
        );
    }
}
