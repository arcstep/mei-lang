//! AST → Decl IR / v2 graph lowering.

pub use mei_surface::{
    desugar_call_name, lower_file, lower_source, lower_source_file, LowerError, LowerOutcome,
};
pub use mei_syntax::{parse_source, parse_source_file, ParseError, SourceFile};
pub use mei_graph::{
    compile_app, CompileAppError, CompileOutcome, GraphBlock, GraphOutcome,
};

pub fn lower_path(path: &std::path::Path) -> Result<LowerOutcome, LowerError> {
    lower_source_file(path)
}
