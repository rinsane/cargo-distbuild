use anyhow::{Context, Result};
use std::io::Read;  // Add this import at the top
use clap::Parser;
// use serde::{Deserialize, Serialize};
use colored::*;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use std::{fs, path::PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;
use reqwest::Client;
use std::fs::File;
use std::io::Write;
use tar::Builder;

#[derive(Parser, Debug)]
#[command(name = "cargo-distbuild")]
#[command(about = "Distributed Rust build tool (crate-level parallelism)")]
struct Args {
    /// Optional path to Cargo.toml of the project
    #[arg(long)]
    manifest_path: Option<PathBuf>,
}

// Need to use this somewhere
#[derive(Debug, Deserialize)]
#[allow(unused)]
struct NodeConfig {
    ip: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<Package>,
    resolve: Resolve,
}

// Need to use this somewhere
#[derive(Debug, Deserialize)]
struct Package {
    id: String,
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct Resolve {
    nodes: Vec<ResolveNode>,
}

#[derive(Debug, Deserialize)]
struct ResolveNode {
    id: String,
    dependencies: Vec<String>,
}

fn debug_tarball_contents(buffer: &[u8]) -> Result<()> {
    use std::io::Read;
    let mut archive = tar::Archive::new(buffer);
    println!("🔍 Tarball contents:");
    
    for entry in archive.entries()? {
        let mut entry = entry?;
        println!("- {}", entry.path()?.display());
        
        // Print first few lines of Cargo.toml
        if entry.path()?.ends_with("Cargo.toml") {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            println!("  Cargo.toml content:\n{}", contents);
        }
    }
    Ok(())
}



async fn compile_crate_on_worker(
    crate_path: &Path,
    worker: &NodeConfig,
    crate_name: &str,
) -> Result<()> {
    let mut buffer = Vec::new();
    
    {
        let mut tar_builder = tar::Builder::new(&mut buffer);
        
        // Add Cargo.toml
        let cargo_toml_path = crate_path.join("Cargo.toml");
        let cargo_content = fs::read_to_string(&cargo_toml_path)?;
        let mut header = tar::Header::new_gnu();
        header.set_path("Cargo.toml")?;
        header.set_size(cargo_content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder.append(&header, cargo_content.as_bytes())?;

        // Add src/lib.rs with explicit UTF-8 validation
        let lib_rs_path = crate_path.join("src/lib.rs");
        let lib_content = fs::read_to_string(&lib_rs_path)
            .context("Failed to read lib.rs (must be valid UTF-8)")?;
        
        let mut header = tar::Header::new_gnu();
        header.set_path("src/lib.rs")?;
        header.set_size(lib_content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder.append(&header, lib_content.as_bytes())?;

        tar_builder.finish()?;
    }

    // Verify tarball contents
    println!("🔍 Verifying tarball contents:");
    let mut archive = tar::Archive::new(&buffer[..]);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        println!("- {}", path.display());
        
        if path.ends_with("lib.rs") {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            println!("  First 50 chars:\n{}", &content[..content.len().min(50)]);
        }
    }

    let client = Client::new();
    let url = format!("http://{}:{}/compile", worker.ip, worker.port);

    let resp = client
        .post(&url)
        .header("Content-Type", "application/octet-stream")
        .body(buffer)
        .send()
        .await
        .context("Failed to send crate to worker")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_body = resp.text().await.unwrap_or_default();
        println!("❌ Worker returned error ({}): {}", status, error_body);
        return Err(anyhow::anyhow!("Worker error: {}", error_body));
    }

    let rlib_bytes = resp.bytes().await?;
    let deps_dir = PathBuf::from("target/debug/deps/");
    fs::create_dir_all(&deps_dir)?;
    let output_path = deps_dir.join(format!("lib{}.rlib", crate_name));
    fs::write(&output_path, &rlib_bytes)?;

    println!("✅ Compiled {} on {}:{}", crate_name, worker.ip, worker.port);
    Ok(())
}

fn read_nodes_config() -> Result<Vec<NodeConfig>> {
    let file = fs::read_to_string("nodes.json").context("Failed to read nodes.json")?;
    let nodes: Vec<NodeConfig> = serde_json::from_str(&file)?;
    Ok(nodes)
}

fn get_cargo_metadata(manifest_path: Option<&Path>) -> Result<CargoMetadata> {
    let mut cmd = Command::new("cargo");
    cmd.arg("metadata").arg("--format-version=1");

    if let Some(path) = manifest_path {
        cmd.arg("--manifest-path").arg(path);
    }

    let output = cmd.output().context("Failed to run cargo metadata")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("cargo metadata failed"));
    }

    let metadata_json = String::from_utf8(output.stdout)?;
    let metadata: CargoMetadata = serde_json::from_str(&metadata_json)?;
    Ok(metadata)
}

fn test_tarball_creation(crate_path: &Path) {
    let mut buffer = Vec::new();
    {
        let mut tar = tar::Builder::new(&mut buffer);
        tar.append_path_with_name(crate_path, "Cargo.toml").unwrap();
    }
    fs::write("test.tar", &buffer).unwrap();
}

#[tokio::main]
async fn main() -> Result<()> {
    // let args = Args::parse();

    println!("{}", "Starting cargo-distbuild...\n".green().bold());

    let nodes = read_nodes_config()?;
    println!("{} {:#?}", "Nodes config loaded:".cyan(), nodes);

    // let metadata = get_cargo_metadata(args.manifest_path.as_deref())?;
    // println!("{} {}", "Found packages:".magenta(), metadata.packages.len());

    // println!("\n{}", "Dependency Graph:".blue().bold());
    // for node in metadata.resolve.nodes {
    //     println!("{} -> {:?}", node.id.to_string().yellow(), node.dependencies);
    // }

    // 👇 TEMPORARY TEST CALL:
    let crate_path = std::fs::canonicalize("../foo")?;
    println!("📂 Absolute crate path: {}", crate_path.display());
    let crate_name = "foo";
    let worker = &nodes[0];

    test_tarball_creation(&crate_path);
    compile_crate_on_worker(&crate_path, worker, crate_name).await?;

    Ok(())
}
