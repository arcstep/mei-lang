#[tokio::main]
async fn main() -> anyhow::Result<()> {
    mei_lang_server::run_cli_for_flavor(mei_lang_server::BinaryFlavor::HostWeb).await
}
