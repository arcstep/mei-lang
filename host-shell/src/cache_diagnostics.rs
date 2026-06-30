const ACCESS_SLIM_ARTIFACTS_ENV: &str = "MEI_ACCESS_SLIM_ARTIFACTS";
const CANONICAL_ARTIFACT_PERSIST_ENV: &str = "MEI_CANONICAL_ARTIFACT_PERSIST";

pub fn access_slim_artifacts_enabled() -> bool {
    true
}

pub fn canonical_artifact_persist_enabled() -> bool {
    true
}

fn access_slim_env_override_detected() -> Option<String> {
    std::env::var(ACCESS_SLIM_ARTIFACTS_ENV).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed == "0" || trimmed.eq_ignore_ascii_case("false") {
            Some(format!("{ACCESS_SLIM_ARTIFACTS_ENV}={trimmed}"))
        } else {
            None
        }
    })
}

fn canonical_artifact_persist_env_override_detected() -> Option<String> {
    std::env::var(CANONICAL_ARTIFACT_PERSIST_ENV).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed == "0" || trimmed.eq_ignore_ascii_case("false") {
            Some(format!("{CANONICAL_ARTIFACT_PERSIST_ENV}={trimmed}"))
        } else {
            None
        }
    })
}

pub fn locked_cache_env_overrides() -> Vec<String> {
    [access_slim_env_override_detected(), canonical_artifact_persist_env_override_detected()]
        .into_iter()
        .flatten()
        .collect()
}
