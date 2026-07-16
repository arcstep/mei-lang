use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct V2SourceFile {
    pub items: Vec<V2Item>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum V2Item {
    ModuleConst {
        name: String,
        value: V2Expr,
    },
    UseTemplate {
        path: String,
        alias: Option<String>,
    },
    TemplateDecl {
        name: String,
        params: Vec<TemplateParam>,
        body: V2Expr,
    },
    TopLevel {
        name: String,
        args: CallArgs,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateParam {
    pub name: String,
    pub default: Option<V2Expr>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallArgs {
    pub positional: Vec<V2Expr>,
    pub keywords: Vec<(String, V2Expr)>,
}

impl CallArgs {
    pub fn empty() -> Self {
        Self {
            positional: Vec::new(),
            keywords: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum V2Expr {
    String(String),
    Number(f64),
    Bool(bool),
    None,
    List(Vec<V2Expr>),
    Dict(Vec<(String, V2Expr)>),
    BinOp {
        op: BinOp,
        left: Box<V2Expr>,
        right: Box<V2Expr>,
    },
    VarRef(String),
    Call {
        path: Vec<String>,
        args: CallArgs,
    },
    RefCall {
        name: String,
        args: CallArgs,
    },
    Member {
        object: Box<V2Expr>,
        field: String,
    },
    ForIn {
        var: String,
        source: Box<V2Expr>,
        body: Box<V2Expr>,
    },
    EnumMatch {
        subject: Box<V2Expr>,
        cases: Vec<(V2Expr, V2Expr)>,
        default: Option<Box<V2Expr>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Merge,
}

pub const V2_TOP_LEVEL_CONSTRUCTORS: &[&str] = &[
    "app_skeleton",
    "navigation",
    "scene",
    "presentation",
    "plane_layout",
    "region_layout",
    "section_layout",
    "slide_layout",
    "content_panel",
    "map_spec",
    "view_spec",
    "metric_def_bundle",
    "page_instance",
    "link_decl",
    "warmup_policy",
    "world",
    "object",
    "object_catalog",
];

pub const V2_REF_KEYWORDS: &[&str] = &[
    "plane_ref",
    "region_ref",
    "section_ref",
    "slide_ref",
    "panel_ref",
    "metric_ref",
    "assembly_ref",
    "link_ref",
    "world_ref",
    "map_ref",
    "view_ref",
    "template_ref",
    "asset_ref",
    "source_ref",
    "theme_ref",
    "metric_bundle_ref",
    "explain_ref",
    "ops_param_ref",
    "board_ref",
    "param_ref",
    "dataset_ref",
    "dataframe_ref",
    "field_ref",
    "entity_ref",
    "stock_ref",
    "source_feature_ref",
    "feature_ref",
    "object_ref",
];

/// Controlled slide_pattern enum (0406).
pub const SLIDE_PATTERNS: &[&str] = &[
    "full_bleed",
    "claim_explain_evidence_action",
    "claim_evidence",
    "three_columns",
    "process",
    "matrix",
];

pub fn slide_pattern_areas(pattern: &str) -> Option<&'static [&'static str]> {
    match pattern {
        "full_bleed" => Some(&["hero"]),
        "claim_explain_evidence_action" => Some(&["claim", "explain", "evidence", "action"]),
        "claim_evidence" => Some(&["claim", "evidence"]),
        "three_columns" => Some(&["col_a", "col_b", "col_c"]),
        "process" => Some(&["title", "steps", "visual"]),
        "matrix" => Some(&["title", "q1", "q2", "q3", "q4"]),
        _ => None,
    }
}
