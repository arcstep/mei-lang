//! Artifact/layer hit matrix for host observability.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactHitMatrix {
    #[serde(default)]
    pub structure_hit: bool,
    #[serde(default)]
    pub eval_hit: bool,
    #[serde(default)]
    pub theme_hit: bool,
    #[serde(default)]
    pub overlay_hit: bool,
    #[serde(default)]
    pub shell_hit: bool,
}

impl ArtifactHitMatrix {
    pub fn summary_tag(&self) -> String {
        format!(
            "structure={} eval={} theme={} overlay={} shell={}",
            hit(self.structure_hit),
            hit(self.eval_hit),
            hit(self.theme_hit),
            hit(self.overlay_hit),
            hit(self.shell_hit),
        )
    }
}

fn hit(value: bool) -> &'static str {
    if value { "hit" } else { "miss" }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LayerArtifactObservability {
    pub hits: ArtifactHitMatrix,
}

impl LayerArtifactObservability {
    pub fn response_headers(&self) -> [(&'static str, String); 5] {
        [
            (
                "x-mei-structure-hit",
                bool_header(self.hits.structure_hit),
            ),
            ("x-mei-eval-hit", bool_header(self.hits.eval_hit)),
            ("x-mei-theme-hit", bool_header(self.hits.theme_hit)),
            (
                "x-mei-overlay-hit",
                bool_header(self.hits.overlay_hit),
            ),
            ("x-mei-shell-hit", bool_header(self.hits.shell_hit)),
        ]
    }
}

fn bool_header(value: bool) -> String {
    if value { "1" } else { "0" }.to_string()
}

pub fn parse_artifact_hits_from_headers(
    headers: &axum::http::HeaderMap,
) -> ArtifactHitMatrix {
    ArtifactHitMatrix {
        structure_hit: header_bool(headers, "x-mei-structure-hit"),
        eval_hit: header_bool(headers, "x-mei-eval-hit"),
        theme_hit: header_bool(headers, "x-mei-theme-hit"),
        overlay_hit: header_bool(headers, "x-mei-overlay-hit"),
        shell_hit: header_bool(headers, "x-mei-shell-hit"),
    }
}

fn header_bool(headers: &axum::http::HeaderMap, name: &str) -> bool {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_tag_lists_layers() {
        let matrix = ArtifactHitMatrix {
            structure_hit: true,
            eval_hit: false,
            theme_hit: true,
            overlay_hit: false,
            shell_hit: true,
        };
        assert!(matrix.summary_tag().contains("structure=hit"));
        assert!(matrix.summary_tag().contains("eval=miss"));
    }
}
