#![allow(unused_imports)]
use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use reqwest::Client;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use tar::Builder;
use tokio::task;
use walkdir;
use toml_edit;

#[derive(Parser, Debug)]
#[command(name = "cargo-distbuild")]
#[command(about = "Distributed Rust build tool")]
struct Args {
    #[arg(long)]
    manifest_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Clone)]
struct NodeConfig {
    ip: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<Package>,
    resolve: Resolve,
    target_directory: String,
}

#[derive(Debug, Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: String,
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    println!("{}", "🚀 Starting cargo-distbuild".green().bold());

    let nodes = read_nodes_config()?;
    let client = Client::new();

    let metadata = get_cargo_metadata(args.manifest_path.as_deref())?;
    let workspace_root = args.manifest_path
        .as_ref()
        .map(|p| p.parent().unwrap().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let target_dir = PathBuf::from(&metadata.target_directory);
    let packages: HashMap<String, (String, PathBuf)> = metadata
        .packages
        .iter()
        .map(|p| {
            (
                p.id.clone(),
                (p.name.clone(), PathBuf::from(&p.manifest_path)),
            )
        })
        .collect();

    let stages = build_dependency_stages(&metadata)?;

    // Track compiled crates and their rlib file paths
    let mut built_rlibs: HashMap<String, PathBuf> = HashMap::new();
    let mut node_idx = 0;

    for stage in stages {
        println!("\n🔨 Compiling stage with {} crates", stage.len());
        let mut tasks = Vec::new();

        for pkg_id in stage {
            let (name, _) = packages.get(&pkg_id).unwrap();
            let worker = nodes[node_idx % nodes.len()].clone();
            node_idx += 1;

            let dependencies: Vec<_> = metadata
                .resolve
                .nodes
                .iter()
                .find(|n| n.id == pkg_id)
                .map(|node| {
                    node.dependencies
                        .iter()
                        .filter_map(|dep_id| {
                            packages.get(dep_id).map(|(n, _)| {
                                let rlib_path = built_rlibs.get(n)?;
                                Some((n.clone(), rlib_path.clone()))
                            })?
                        })
                        .collect()
                })
                .unwrap_or_default();

            let name = name.clone();
            let target_dir = target_dir.clone();
            let client = client.clone();
            let workspace_root = workspace_root.clone();

            tasks.push(task::spawn(async move {
                let (filename, binary) = compile_task(
                    client,
                    worker,
                    &name,
                    &workspace_root,
                    &dependencies,
                )
                .await?;

                let output_path = if filename.ends_with(".rlib") {
                    // For libraries, store in deps directory
                    let deps_dir = target_dir.join("debug/deps");
                    fs::create_dir_all(&deps_dir)?;
                    let path = deps_dir.join(&filename);
                    fs::write(&path, &binary)?;
                    println!("✅ Stored {} to {}", name, path.display());
                    path
                } else {
                    // For binaries, store in debug directory
                    let debug_dir = target_dir.join("debug");
                    fs::create_dir_all(&debug_dir)?;
                    let path = debug_dir.join(&filename);
                    fs::write(&path, &binary)?;
                    println!("✅ Stored {} to {}", name, path.display());
                    path
                };

                Ok::<_, anyhow::Error>((name, output_path))
            }));
        }

        for result in futures::future::join_all(tasks).await {
            match result {
                Ok(Ok((crate_name, path))) => {
                    built_rlibs.insert(crate_name, path);
                }
                Ok(Err(e)) => eprintln!("❌ Worker failed: {e:?}"),
                Err(e) => eprintln!("❌ Task join error: {e:?}"),
            }
        }
    }

    println!("\n🎯 All crates compiled. Run with:");
    println!("   {}", "cargo run -p <binary> --offline".yellow());

    Ok(())
}

fn build_dependency_stages(metadata: &CargoMetadata) -> Result<Vec<Vec<String>>> {
    let mut graph = HashMap::new();
    let mut in_degree = HashMap::new();

    for node in &metadata.resolve.nodes {
        graph.insert(node.id.clone(), Vec::new());
        in_degree.insert(node.id.clone(), 0);
    }

    for node in &metadata.resolve.nodes {
        for dep in &node.dependencies {
            graph.get_mut(dep).unwrap().push(node.id.clone());
            *in_degree.get_mut(&node.id).unwrap() += 1;
        }
    }

    let mut stages = Vec::new();
    let mut queue: Vec<String> = in_degree
        .iter()
        .filter_map(|(id, &deg)| if deg == 0 { Some(id.clone()) } else { None })
        .collect();

    while !queue.is_empty() {
        let mut next = Vec::new();
        for id in &queue {
            for dependent in graph.get(id).unwrap_or(&Vec::new()) {
                let count = in_degree.get_mut(dependent).unwrap();
                *count -= 1;
                if *count == 0 {
                    next.push(dependent.clone());
                }
            }
        }
        stages.push(queue);
        queue = next;
    }

    Ok(stages)
}

fn read_nodes_config() -> Result<Vec<NodeConfig>> {
    let content = fs::read_to_string("nodes.json")?;
    Ok(serde_json::from_str(&content)?)
}

fn get_cargo_metadata(manifest_path: Option<&Path>) -> Result<CargoMetadata> {
    let mut cmd = Command::new("cargo");
    cmd.arg("metadata").arg("--format-version=1");

    if let Some(path) = manifest_path {
        cmd.arg("--manifest-path").arg(path);
    }

    let output = cmd.output()?;
    if !output.status.success() {
        return Err(anyhow::anyhow!("cargo metadata failed"));
    }

    Ok(serde_json::from_slice(&output.stdout)?)
}

fn print_tarball_contents(buffer: &[u8]) -> Result<()> {
    use std::io::{Read, Cursor};
    let mut archive = tar::Archive::new(Cursor::new(buffer));
    println!("📦 Contents of generated tarball:");

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        println!("- {}", path.display());

        if path.ends_with("Cargo.toml") {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            println!(
                "📝 Cargo.toml:\n{}",
                contents.lines().take(5).collect::<Vec<_>>().join("\n")
            );
        }
    }

    Ok(())
}

async fn compile_task(
    client: Client,
    worker: NodeConfig,
    crate_name: &str,
    workspace_root: &Path,
    _dependencies: &[(String, PathBuf)],
) -> Result<(String, Vec<u8>)> {
    println!("📦 Creating tarball for {}:", crate_name);

    let mut buffer = Vec::new();
    {
        let mut tar = Builder::new(&mut buffer);

        // Add workspace Cargo.toml
        let workspace_cargo = workspace_root.join("Cargo.toml");
        if !workspace_cargo.exists() {
            return Err(anyhow::anyhow!("Workspace Cargo.toml not found"));
        }
        let mut header = tar::Header::new_gnu();
        let cargo_bytes = fs::read(&workspace_cargo)?;
        header.set_size(cargo_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "Cargo.toml", cargo_bytes.as_slice())?;

        // Add all crates directories
        let crates_dir = workspace_root.join("crates");
        println!("   Adding workspace crates from: {}", crates_dir.display());
        for entry in walkdir::WalkDir::new(&crates_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let relative_path = entry.path().strip_prefix(workspace_root)?;
            let path_str = relative_path.display().to_string();
            
            if entry.file_type().is_file() {
                println!("   - {} (reading file...)", path_str);
                let file_contents = fs::read(entry.path())?;
                println!("     Read {} bytes", file_contents.len());
                
                let mut header = tar::Header::new_gnu();
                header.set_size(file_contents.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                
                tar.append_data(&mut header, &path_str, &file_contents[..])?;
                println!("     Added to tarball");
            } else if entry.file_type().is_dir() {
                let mut header = tar::Header::new_gnu();
                header.set_entry_type(tar::EntryType::Directory);
                header.set_mode(0o755);
                header.set_size(0);
                header.set_cksum();
                tar.append_data(&mut header, &path_str, &[][..])?;
            }
        }

        tar.finish()?;
    }

    println!("   Tarball size: {} bytes", buffer.len());

    // Print tarball contents for debugging
    print_tarball_contents(&buffer)?;

    // Send to worker with crate name parameter
    let url = format!("http://{}:{}/compile?crate_name={}", worker.ip, worker.port, crate_name);
    println!("   Sending to worker: {}", url);
    
    let response = client
        .post(&url)
        .header("Content-Type", "application/octet-stream")
        .body(buffer)
        .send()
        .await?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Worker error: {}", error));
    }

    // Check headers before consuming response
    let is_binary = response.headers().contains_key("X-Binary-File");
    
    let binary = response.bytes().await?.to_vec();
    let filename = if is_binary {
        crate_name.to_string()
    } else {
        format!("lib{}.rlib", crate_name)
    };
    
    println!("   Received compiled output: {} ({} bytes)", filename, binary.len());
    
    Ok((filename, binary))
}
