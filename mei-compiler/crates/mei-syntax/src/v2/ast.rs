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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
}

pub const V2_TOP_LEVEL_CONSTRUCTORS: &[&str] = &[
    "app_skeleton",
    "navigation",
    "assembly_view",
    "panel_contract",
    "metric_def_bundle",
    "board_assembly",
    "link_decl",
    "warmup_policy",
];

pub const V2_REF_KEYWORDS: &[&str] = &[
    "panel_ref",
    "metric_ref",
    "assembly_ref",
    "link_ref",
    "template_ref",
    "asset_ref",
    "source_ref",
    "theme_ref",
    "metric_bundle_ref",
    "explain_ref",
    "ops_param_ref",
    "board_ref",
    "param_ref",
    "link_ref",
];
