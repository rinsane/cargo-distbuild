use anyhow::{Context, Result};
use clap::Parser;
// use serde::{Deserialize, Serialize};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use std::{fs, path::PathBuf};
use colored::*;

use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::Client;
// use std::io::Write;
use std::fs::File;

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
#[allow(unused)]
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

/// Packages a crate's source code and sends it to a worker for compilation.
async fn compile_crate_on_worker(
    crate_path: &Path,
    worker: &NodeConfig,
    crate_name: &str,
) -> Result<()> {
    let tar_gz_path = format!("{}.tar.gz", crate_name);
    let tar_gz_file = File::create(&tar_gz_path)?;
    let enc = GzEncoder::new(tar_gz_file, Compression::default());
    let mut tar_builder = tar::Builder::new(enc);

    // Include Cargo.toml and src/
    tar_builder.append_path(crate_path.join("Cargo.toml"))?;
    tar_builder.append_dir_all("src", crate_path.join("src"))?;
    tar_builder.finish()?;
    drop(tar_builder);

    let file_bytes = fs::read(&tar_gz_path)?;
    let client = Client::new();
    let url = format!("http://{}:{}/compile", worker.ip, worker.port);

    let resp = client
        .post(&url)
        .body(file_bytes)
        .header("Content-Type", "application/octet-stream")
        .send()
        .await
        .context("Failed to send crate to worker")?;

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "Worker returned non-200: {}",
            resp.status()
        ));
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("{}", "Starting cargo-distbuild...\n".green().bold());

    let nodes = read_nodes_config()?;
    println!("{} {:#?}", "Nodes config loaded:".cyan(), nodes);

    let metadata = get_cargo_metadata(args.manifest_path.as_deref())?;
    println!("{} {}", "Found packages:".magenta(), metadata.packages.len());

    println!("\n{}", "Dependency Graph:".blue().bold());
    for node in metadata.resolve.nodes {
        println!("{} -> {:?}", node.id.to_string().yellow(), node.dependencies);
    }

    // 👇 TEMPORARY TEST CALL:
    let crate_path = Path::new("../foo"); // or absolute path
    let crate_name = "foo";
    let worker = &nodes[0];

    compile_crate_on_worker(crate_path, worker, crate_name).await?;

    Ok(())
}
