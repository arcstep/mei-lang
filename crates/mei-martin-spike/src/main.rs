//! Feasibility spike for embedding / supervising Martin with mei-lang.
//!
//! Modes:
//! - `library` — use `martin-core` `MbtSource` to open MBTiles + fetch one tile
//! - `subprocess` — spawn `martin` on a random loopback port over a tiles directory
//!
//! Not part of the product build; run with:
//!   cargo run --manifest-path crates/mei-martin-spike/Cargo.toml -- library
//!   cargo run --manifest-path crates/mei-martin-spike/Cargo.toml -- subprocess

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use martin_core::tiles::mbtiles::MbtSource;
use martin_core::tiles::Source;
use martin_core::CacheZoomRange;
use martin_tile_utils::TileCoord;
use tokio::process::Command;
use tokio::time::{sleep, Instant};

fn default_mbtiles() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2/stock/gis/tiles/huale-z10-16.mbtiles")
}

fn default_tiles_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2/stock/gis/tiles")
}

fn resolve_martin_bin() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("MEI_MARTIN_BIN") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Ok(path);
        }
        bail!("MEI_MARTIN_BIN is not a file: {}", path.display());
    }
    for candidate in [
        "/opt/homebrew/bin/martin",
        "/usr/local/bin/martin",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Ok(path);
        }
    }
    // Fall back to PATH
    which_martin().context("martin binary not found; set MEI_MARTIN_BIN or install martin")
}

fn which_martin() -> Result<PathBuf> {
    let output = std::process::Command::new("which")
        .arg("martin")
        .output()
        .context("run which martin")?;
    if !output.status.success() {
        bail!("which martin failed");
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() {
        bail!("empty which martin");
    }
    Ok(PathBuf::from(s))
}

fn reserve_loopback_port() -> Result<u16> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .context("bind 127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

async fn run_library(mbtiles: &Path) -> Result<()> {
    println!("==> [library] open {}", mbtiles.display());
    if !mbtiles.is_file() {
        bail!("mbtiles missing: {}", mbtiles.display());
    }
    let id = mbtiles
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("spike")
        .to_string();
    let source = MbtSource::new(id.clone(), mbtiles.to_path_buf(), CacheZoomRange::default())
        .await
        .map_err(|e| anyhow!("MbtSource::new: {e}"))?;

    let tj = source.get_tilejson();
    println!(
        "    id={} minzoom={:?} maxzoom={:?} tiles={:?}",
        source.get_id(),
        tj.minzoom,
        tj.maxzoom,
        tj.tiles
    );

    // huale-z10-16 stores TMS rows; Martin/Source expects XYZ.
    // Known present tile from sqlite: z=10 col=834 row_tms=579 → xyz_y = 2^10-1-579 = 444
    let candidates = [
        TileCoord {
            z: 10,
            x: 834,
            y: 444,
        },
        TileCoord {
            z: 11,
            x: 1668,
            y: 888,
        }, // 2^11-1-1159 = 888
    ];
    let mut got = None;
    for coord in candidates {
        match source.get_tile(coord, None).await {
            Ok(data) if !data.is_empty() => {
                println!(
                    "    get_tile z={} x={} y={} -> {} bytes",
                    coord.z,
                    coord.x,
                    coord.y,
                    data.len()
                );
                got = Some(data.len());
                break;
            }
            Ok(_) => {
                println!(
                    "    get_tile z={} x={} y={} -> empty",
                    coord.z, coord.x, coord.y
                );
            }
            Err(e) => {
                println!(
                    "    get_tile z={} x={} y={} -> err: {e}",
                    coord.z, coord.x, coord.y
                );
            }
        }
    }
    if got.is_none() {
        bail!("library path: no non-empty tile returned");
    }
    println!("==> [library] OK");
    Ok(())
}

async fn wait_catalog(base: &str, child: &mut tokio::process::Child) -> Result<()> {
    let url = format!("{base}/catalog");
    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait()? {
            bail!("martin exited during startup: {status}");
        }
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let body: serde_json::Value = resp.json().await.context("parse catalog json")?;
                println!("    catalog keys: {}", summarize_catalog(&body));
                return Ok(());
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            bail!("catalog timeout at {url}");
        }
        sleep(Duration::from_millis(200)).await;
    }
}

fn summarize_catalog(body: &serde_json::Value) -> String {
    if let Some(tiles) = body.get("tiles").and_then(|t| t.as_object()) {
        let keys: Vec<_> = tiles.keys().cloned().collect();
        return format!("{:?}", keys);
    }
    body.to_string().chars().take(200).collect()
}

async fn run_subprocess(tiles_dir: &Path) -> Result<()> {
    println!("==> [subprocess] tiles dir {}", tiles_dir.display());
    if !tiles_dir.is_dir() {
        bail!("tiles dir missing: {}", tiles_dir.display());
    }
    let bin = resolve_martin_bin()?;
    let port = reserve_loopback_port()?;
    let listen = format!("127.0.0.1:{port}");
    let base = format!("http://{listen}");
    println!("    binary={}", bin.display());
    println!("    listen={listen} (random reserved port)");

    let mut child = Command::new(&bin)
        .arg("--listen-addresses")
        .arg(&listen)
        .arg(tiles_dir.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("spawn {}", bin.display()))?;

    wait_catalog(&base, &mut child).await?;

    // Fetch TileJSON for huale source id
    let source_id = "huale-z10-16";
    let tj_url = format!("{base}/{source_id}");
    let client = reqwest::Client::new();
    let tj_resp = client
        .get(&tj_url)
        .send()
        .await
        .with_context(|| format!("GET {tj_url}"))?;
    if !tj_resp.status().is_success() {
        bail!("TileJSON status {} for {tj_url}", tj_resp.status());
    }
    let tj: serde_json::Value = tj_resp.json().await.context("tilejson body")?;
    println!(
        "    TileJSON id={source_id} minzoom={:?} maxzoom={:?} tiles={:?}",
        tj.get("minzoom"),
        tj.get("maxzoom"),
        tj.get("tiles")
    );

    // Probe a tile URL pattern from TileJSON if present
    if let Some(tiles) = tj.get("tiles").and_then(|t| t.as_array()) {
        if let Some(template) = tiles.first().and_then(|t| t.as_str()) {
            let sample = template
                .replace("{z}", "10")
                .replace("{x}", "834")
                .replace("{y}", "444");
            let tile_resp = client.get(&sample).send().await.context("GET sample tile")?;
            let bytes = tile_resp.bytes().await.context("tile body")?;
            println!(
                "    sample tile {} -> {} bytes",
                sample,
                bytes.len()
            );
            if bytes.is_empty() {
                bail!("sample tile empty");
            }
        }
    }

    let _ = child.start_kill();
    let _ = child.wait().await;
    println!("==> [subprocess] OK (killed child)");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "both".into());
    let path_arg = args.next().map(PathBuf::from);

    match mode.as_str() {
        "library" => {
            let mbtiles = path_arg.unwrap_or_else(default_mbtiles);
            run_library(&mbtiles).await?;
        }
        "subprocess" => {
            let tiles = path_arg.unwrap_or_else(default_tiles_dir);
            run_subprocess(&tiles).await?;
        }
        "both" => {
            let mbtiles = default_mbtiles();
            let tiles = default_tiles_dir();
            run_library(&mbtiles).await?;
            run_subprocess(&tiles).await?;
        }
        other => bail!("unknown mode {other}; use library|subprocess|both"),
    }
    Ok(())
}
