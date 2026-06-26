#!/usr/bin/env python3
"""Fix Phase 3 kernel mod.rs glob re-export visibility warnings."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "src/compile"

FINISH_MOD = """mod assemble;
mod projection;
mod projection_tree;
mod hydrate;

pub(in crate::compile::app_compile) use assemble::{finish_compiled_app, CompileCacheBefore};
use projection::*;
use projection_tree::*;
use hydrate::*;
"""

BUILD_EXPERIENCE_INDEX_MOD = """mod index;
mod reachability;
mod rebuild;
mod tree;

#[cfg(test)]
mod tests;

pub use index::*;
pub use reachability::*;
use rebuild::*;
use tree::*;
"""

BUILD_NODE_CONTEXT_MOD = """mod preview;
mod context;
mod helpers;

#[cfg(test)]
mod tests;

pub use preview::*;
pub use context::*;
use helpers::*;
"""

PANEL_MOD = """mod link;
mod params;
mod shell;
mod shell_zones;
mod preview;

use params::*;
use shell::*;
use shell_zones::*;

pub(crate) use preview::enrich_scene_projection_assembly_preview;
pub(crate) use link::lower_scene_links_in_panels;
pub(crate) use shell::scene_shell_contract_from_scene_contract;
"""

METRIC_MOD = """mod expand_core;
mod expand_slots;
mod views;
mod explain;
mod drilldown;
mod slots;

use expand_core::*;
use expand_slots::*;
use views::*;
use explain::*;
use drilldown::*;
use slots::*;

pub(crate) use expand_core::expand_board_assembly;
pub(crate) use drilldown::expand_drilldown_tabs;
pub(crate) use slots::build_generic_rowset_filter_schema;
"""


def write(rel: str, content: str) -> None:
    path = ROOT / rel
    path.write_text(content, encoding="utf-8")
    print(f"  {path.relative_to(ROOT.parents[1])}")


def main() -> None:
    write("app_compile/finish/mod.rs", FINISH_MOD)
    write("build_experience_index/mod.rs", BUILD_EXPERIENCE_INDEX_MOD)
    write("build_node_context/mod.rs", BUILD_NODE_CONTEXT_MOD)
    write("projection_assembly/panel/mod.rs", PANEL_MOD)
    write("projection_assembly/metric/mod.rs", METRIC_MOD)
    print("done")


if __name__ == "__main__":
    main()
