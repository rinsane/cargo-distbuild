use anyhow::{Result, Context};
use std::path::Path;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::{fs, path::PathBuf};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "cargo-distbuild")]
#[command(about = "Distributed Rust build tool (crate-level parallelism)")]
struct Args {
    /// Optional path to Cargo.toml of the project
    #[arg(long)]
    manifest_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct NodeConfig {
    ip: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<Package>,
    resolve: Resolve,
}

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

fn main() -> Result<()> {
    let args = Args::parse();

    println!("🚀 Starting cargo-distbuild...\n");

    let nodes = read_nodes_config()?;
    println!("📡 Nodes config loaded: {:#?}", nodes);

    let metadata = get_cargo_metadata(args.manifest_path.as_deref())?;
    println!("📦 Found {} packages", metadata.packages.len());

    println!("\n🧱 Dependency Graph:");
    for node in metadata.resolve.nodes {
        println!("{} -> {:?}", node.id, node.dependencies);
    }

    Ok(())
}
