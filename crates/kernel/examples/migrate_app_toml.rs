//! One-shot: migrate apps/{id}/{app.config.json,launch.json} → app.toml
//!
//!   cargo run -p mei-lang-kernel --example migrate_app_toml -- \
//!     /path/to/ws-demo-v2 mini-grid metric-grid ...

use std::env;
use std::path::PathBuf;

use mei_lang_kernel::migrate_json_pair_to_toml;

fn main() {
    let mut args = env::args().skip(1);
    let workspace = PathBuf::from(args.next().expect("workspace root"));
    let apps: Vec<String> = args.collect();
    if apps.is_empty() {
        eprintln!("usage: migrate_app_toml <workspace> <app_id>...");
        std::process::exit(2);
    }
    for app_id in apps {
        let root = workspace.join("apps").join(&app_id);
        match migrate_json_pair_to_toml(&root) {
            Ok(()) => println!("ok  {}", root.display()),
            Err(e) => {
                eprintln!("err {}: {}", root.display(), e);
                std::process::exit(1);
            }
        }
    }
}
