use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceFile {
    pub statements: Vec<TopLevelCall>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopLevelCall {
    pub path: Vec<String>,
    pub args: CallArgs,
    pub span_start: usize,
    pub span_end: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallArgs {
    pub positional: Vec<Expr>,
    pub keywords: Vec<(String, Expr)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    String(String),
    Number(f64),
    Bool(bool),
    None,
    List(Vec<Expr>),
    Call {
        path: Vec<String>,
        args: CallArgs,
    },
}

impl CallArgs {
    pub fn empty() -> Self {
        Self {
            positional: Vec::new(),
            keywords: Vec::new(),
        }
    }
}
