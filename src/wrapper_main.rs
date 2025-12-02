// Wrapper binary entry point
// This is what Cargo will call instead of rustc

#[tokio::main]
async fn main() {
    if let Err(e) = cargo_distbuild::wrapper::run_wrapper().await {
        eprintln!("cargo-distbuild wrapper error: {}", e);
        std::process::exit(1);
    }
}

