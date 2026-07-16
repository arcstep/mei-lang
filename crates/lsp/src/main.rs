mod backend;
mod diagnostics;
mod source_index;

use anyhow::Result;
use backend::Backend;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    if matches!(
        args.next().as_deref(),
        Some(value) if value == "-V" || value == "--version"
    ) && args.next().is_none()
    {
        println!("mei-lsp {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .compact()
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}
