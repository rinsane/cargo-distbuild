use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::process::Command;

pub mod rustc_parser;

use rustc_parser::RustcArgs;

/// Main entry point for the wrapper
/// Called by Cargo instead of rustc
pub async fn run_wrapper() -> Result<()> {
    // Get all arguments passed by Cargo
    let args: Vec<String> = env::args().collect();
    
    // args[0] is our binary name
    // args[1..] are the rustc arguments
    if args.len() < 2 {
        eprintln!("cargo-distbuild wrapper: No arguments provided");
        std::process::exit(1);
    }

    // Check if this is a query/check operation (should run locally)
    if should_run_locally(&args[1..]) {
        return run_local_rustc(&args[1..]);
    }

    // Parse rustc arguments
    let rustc_args = match RustcArgs::parse(&args[1..]) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("cargo-distbuild wrapper: Failed to parse rustc args: {}", e);
            eprintln!("Falling back to local compilation");
            return run_local_rustc(&args[1..]);
        }
    };

    // For now, if it's not a library compilation, run locally
    if !rustc_args.is_lib {
        return run_local_rustc(&args[1..]);
    }

    eprintln!("🚀 [cargo-distbuild] Intercepted rustc call for crate: {:?}", rustc_args.crate_name);
    eprintln!("   Output: {:?}", rustc_args.output_path);

    // Try distributed compilation
    match compile_distributed(&rustc_args).await {
        Ok(_) => {
            eprintln!("✅ [cargo-distbuild] Distributed compilation successful");
            Ok(())
        }
        Err(e) => {
            eprintln!("⚠️  [cargo-distbuild] Distributed compilation failed: {}", e);
            eprintln!("   Falling back to local compilation");
            run_local_rustc(&args[1..])
        }
    }
}

/// Check if we should skip distributed compilation for this invocation
fn should_run_locally(args: &[String]) -> bool {
    // Run locally for:
    // - Version queries: --version, --print
    // - Help: --help
    // - Build scripts (build.rs)
    // - Proc macros
    
    for arg in args {
        if arg == "--version" 
            || arg == "--help"
            || arg.starts_with("--print")
            || arg.contains("build_script_build")
            || arg.contains("proc-macro")
        {
            return true;
        }
    }
    
    false
}

/// Run rustc locally (fallback)
fn run_local_rustc(args: &[String]) -> Result<()> {
    let status = Command::new("rustc")
        .args(args)
        .status()
        .context("Failed to execute rustc")?;
    
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    
    Ok(())
}

/// Compile on the distributed system
async fn compile_distributed(rustc_args: &RustcArgs) -> Result<()> {
    use crate::cas::Cas;
    use crate::common::Config;
    use crate::proto::distbuild::scheduler_client::SchedulerClient;
    use crate::proto::distbuild::*;
    
    // Load config
    let config = Config::load_default()?;
    let cas = Cas::new(&config.cas.root)?;
    
    eprintln!("📦 [cargo-distbuild] Packaging source files for CAS...");
    
    // Create a tarball of the crate source
    let tarball = create_source_tarball(rustc_args)?;
    
    // Upload to CAS
    let input_hash = cas.put(&tarball)?;
    eprintln!("   Input hash: {}", &input_hash[..16]);
    
    // Connect to scheduler
    let scheduler_addr = format!("http://{}", config.scheduler.addr);
    let mut client = SchedulerClient::connect(scheduler_addr)
        .await
        .context("Failed to connect to scheduler")?;
    
    // Submit job
    let job_id = uuid::Uuid::new_v4().to_string();
    let request = SubmitJobRequest {
        job_id: job_id.clone(),
        input_hash: input_hash.clone(),
        job_type: "rust-compile".to_string(),
        metadata: std::collections::HashMap::from([
            ("crate_name".to_string(), rustc_args.crate_name.clone().unwrap_or_default()),
            ("rustc_args".to_string(), rustc_args.original_args.join(" ")),
        ]),
    };
    
    eprintln!("📤 [cargo-distbuild] Submitting job to scheduler...");
    client.submit_job(request).await?;
    
    // Poll for completion
    eprintln!("⏳ [cargo-distbuild] Waiting for compilation...");
    let output_hash = poll_for_completion(&mut client, &job_id).await?;
    
    // Download output from CAS
    eprintln!("📥 [cargo-distbuild] Downloading output...");
    let output_data = cas.get(&output_hash)?;
    
    // Write to output location
    if let Some(output_path) = &rustc_args.output_path {
        let size = output_data.len();
        fs::write(output_path, output_data)?;
        eprintln!("   Wrote {} bytes to {:?}", size, output_path);
    }
    
    Ok(())
}

/// Poll scheduler until job completes
async fn poll_for_completion(
    client: &mut crate::proto::distbuild::scheduler_client::SchedulerClient<tonic::transport::Channel>,
    job_id: &str,
) -> Result<String> {
    use crate::proto::distbuild::*;
    use tokio::time::{sleep, Duration};
    
    for attempt in 0..60 {  // Poll for up to 60 seconds
        sleep(Duration::from_secs(1)).await;
        
        let request = GetJobStatusRequest {
            job_id: job_id.to_string(),
        };
        
        let response = client.get_job_status(request).await?;
        let status = response.into_inner();
        
        match status.status {
            3 => {  // COMPLETED
                if status.output_hash.is_empty() {
                    anyhow::bail!("Job completed but no output hash");
                }
                return Ok(status.output_hash);
            }
            4 => {  // FAILED
                anyhow::bail!("Job failed: {}", status.error);
            }
            _ => {
                if attempt % 5 == 0 {
                    eprintln!("   Still waiting... ({}/60s)", attempt);
                }
            }
        }
    }
    
    anyhow::bail!("Job timeout after 60 seconds")
}

/// Create a tarball of source files for the crate
fn create_source_tarball(rustc_args: &RustcArgs) -> Result<Vec<u8>> {
    use tar::Builder;
    
    let mut buffer = Vec::new();
    let mut tar = Builder::new(&mut buffer);
    
    // Add all input .rs files
    for input_file in &rustc_args.input_files {
        if input_file.exists() {
            let file_name = input_file.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("input.rs");
            
            let data = fs::read(input_file)?;
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, file_name, &data[..])?;
        }
    }
    
    // Add metadata file with rustc args
    let metadata = serde_json::json!({
        "crate_name": rustc_args.crate_name,
        "is_lib": rustc_args.is_lib,
        "rustc_args": rustc_args.original_args,
    });
    let metadata_json = serde_json::to_vec_pretty(&metadata)?;
    let mut header = tar::Header::new_gnu();
    header.set_size(metadata_json.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "metadata.json", &metadata_json[..])?;
    
    tar.finish()?;
    drop(tar);
    
    Ok(buffer)
}

