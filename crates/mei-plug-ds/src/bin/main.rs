use clap::{CommandFactory, Parser};

use mei_plug_ds::{run_serve, run_warmup, Cli, Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .compact()
        .init();

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
        Command::Warmup(args) => run_warmup(args).await,
        Command::Serve(args) => run_serve(args).await,
    }
}

fn print_version(json: bool) -> anyhow::Result<()> {
    let payload = serde_json::json!({
        "name": "mei-plug-ds",
        "version": env!("CARGO_PKG_VERSION"),
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("mei-plug-ds {}", env!("CARGO_PKG_VERSION"));
    }
    Ok(())
}
