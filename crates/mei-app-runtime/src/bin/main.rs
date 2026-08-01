use clap::{CommandFactory, Parser};

use mei_app_runtime::{run_serve, Cli, Command};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .compact()
        .init();

    // DataFusion planning for factored pipeline SQL (WITH + UNION ALL + ROW_NUMBER)
    // overflows the default ~2MiB tokio worker stack; keep workers at 8MiB.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("mei-app-runtime")
        .thread_stack_size(8 * 1024 * 1024)
        .build()?;

    runtime.block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if cli.print_version {
        return print_version(false);
    }
    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        return Ok(());
    };
    match command {
        Command::Version(args) => print_version(args.json),
        Command::Serve(args) => run_serve(args).await,
        Command::Warmup(args) => mei_plug_ds::run_warmup(args).await,
    }
}

fn print_version(json: bool) -> anyhow::Result<()> {
    let payload = serde_json::json!({
        "name": "mei-app-runtime",
        "version": env!("CARGO_PKG_VERSION"),
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("mei-app-runtime {}", env!("CARGO_PKG_VERSION"));
    }
    Ok(())
}
