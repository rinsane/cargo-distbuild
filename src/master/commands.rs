use crate::cas::Cas;
use crate::common::Config;
use crate::proto::distbuild::scheduler_client::SchedulerClient;
use crate::proto::distbuild::*;
use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::path::Path;
use uuid::Uuid;

pub struct CommandExecutor {
    config: Config,
    cas: Cas,
}

impl CommandExecutor {
    pub fn new(config: Config) -> Result<Self> {
        let cas = Cas::new(&config.cas.root)?;
        Ok(CommandExecutor { config, cas })
    }

    pub async fn cas_put(&self, file_path: &str) -> Result<()> {
        let path = Path::new(file_path);
        let data = fs::read(path)
            .with_context(|| format!("Failed to read file: {}", file_path))?;

        let hash = self.cas.put(&data)?;
        
        println!("{}", "✅ File stored in CAS".green());
        println!("   File: {}", file_path);
        println!("   Size: {} bytes", data.len());
        println!("   Hash: {}", hash.bright_cyan());

        Ok(())
    }

    pub async fn cas_get(&self, hash: &str, output_path: &str) -> Result<()> {
        let data = self.cas.get(hash)
            .with_context(|| format!("Hash not found in CAS: {}", hash))?;

        fs::write(output_path, &data)
            .with_context(|| format!("Failed to write to: {}", output_path))?;

        println!("{}", "✅ File retrieved from CAS".green());
        println!("   Hash: {}", hash.bright_cyan());
        println!("   Size: {} bytes", data.len());
        println!("   Saved to: {}", output_path);

        Ok(())
    }

    pub async fn cas_exists(&self, hash: &str) -> Result<()> {
        let exists = self.cas.exists(hash);
        
        if exists {
            println!("{} Hash exists in CAS", "✓".green());
        } else {
            println!("{} Hash not found in CAS", "✗".red());
        }
        println!("   Hash: {}", hash.bright_cyan());

        Ok(())
    }

    pub async fn cas_list(&self) -> Result<()> {
        let hashes = self.cas.list_all()?;
        
        println!("{}", format!("📦 CAS contains {} blob(s):", hashes.len()).bold());
        for (i, hash) in hashes.iter().enumerate() {
            println!("  {}. {}", i + 1, hash.bright_cyan());
        }

        Ok(())
    }

    pub async fn submit_job(&self, input_hash: &str) -> Result<()> {
        let scheduler_addr = format!("http://{}", self.config.scheduler.addr);
        let mut client = SchedulerClient::connect(scheduler_addr)
            .await
            .context("Failed to connect to scheduler")?;

        // Check if input exists in CAS
        if !self.cas.exists(input_hash) {
            anyhow::bail!("Input hash {} not found in CAS", input_hash);
        }

        let job_id = Uuid::new_v4().to_string();

        let request = SubmitJobRequest {
            job_id: job_id.clone(),
            input_hash: input_hash.to_string(),
            job_type: "transform".to_string(),
            metadata: std::collections::HashMap::new(),
        };

        let response = client.submit_job(request).await?;
        let resp = response.into_inner();

        if resp.success {
            println!("{}", "✅ Job submitted successfully".green());
            println!("   Job ID: {}", job_id.bright_yellow());
            println!("   Input: {}", input_hash.bright_cyan());
        } else {
            anyhow::bail!("Failed to submit job: {}", resp.message);
        }

        Ok(())
    }

    pub async fn job_status(&self, job_id: &str) -> Result<()> {
        let scheduler_addr = format!("http://{}", self.config.scheduler.addr);
        let mut client = SchedulerClient::connect(scheduler_addr)
            .await
            .context("Failed to connect to scheduler")?;

        let request = GetJobStatusRequest {
            job_id: job_id.to_string(),
        };

        let response = client.get_job_status(request).await?;
        let resp = response.into_inner();

        let status_str = match resp.status {
            0 => "PENDING".yellow(),
            1 => "ASSIGNED".cyan(),
            2 => "RUNNING".blue(),
            3 => "COMPLETED".green(),
            4 => "FAILED".red(),
            _ => "UNKNOWN".white(),
        };

        println!("{}", "📊 Job Status".bold());
        println!("   Job ID: {}", job_id.bright_yellow());
        println!("   Status: {}", status_str);
        
        if !resp.assigned_worker.is_empty() {
            println!("   Worker: {}", resp.assigned_worker);
        }
        
        if !resp.output_hash.is_empty() {
            println!("   Output: {}", resp.output_hash.bright_cyan());
        }
        
        if !resp.error.is_empty() {
            println!("   Error: {}", resp.error.red());
        }

        Ok(())
    }

    pub async fn list_workers(&self) -> Result<()> {
        let scheduler_addr = format!("http://{}", self.config.scheduler.addr);
        let mut client = SchedulerClient::connect(scheduler_addr)
            .await
            .context("Failed to connect to scheduler")?;

        let request = ListWorkersRequest {};
        let response = client.list_workers(request).await?;
        let resp = response.into_inner();

        println!("{}", format!("🔧 Registered Workers ({})", resp.workers.len()).bold());
        
        if resp.workers.is_empty() {
            println!("   {}", "No workers registered".yellow());
        } else {
            for worker in resp.workers {
                let capacity_str = format!("{}/{}", worker.active_jobs, worker.capacity);
                println!("\n  • {}", worker.worker_id.bright_green());
                println!("    Address: {}", worker.address);
                println!("    Load: {}", capacity_str);
                println!("    Last heartbeat: {} seconds ago", 
                    chrono::Utc::now().timestamp() - worker.last_heartbeat);
            }
        }

        Ok(())
    }

    pub async fn list_jobs(&self, limit: u32) -> Result<()> {
        let scheduler_addr = format!("http://{}", self.config.scheduler.addr);
        let mut client = SchedulerClient::connect(scheduler_addr)
            .await
            .context("Failed to connect to scheduler")?;

        let request = ListJobsRequest { limit };
        let response = client.list_jobs(request).await?;
        let resp = response.into_inner();

        println!("{}", format!("📋 Jobs (showing {})", resp.jobs.len()).bold());
        
        if resp.jobs.is_empty() {
            println!("   {}", "No jobs".yellow());
        } else {
            for job in resp.jobs {
                let status_str = match job.status {
                    0 => "PENDING".yellow(),
                    1 => "ASSIGNED".cyan(),
                    2 => "RUNNING".blue(),
                    3 => "COMPLETED".green(),
                    4 => "FAILED".red(),
                    _ => "UNKNOWN".white(),
                };

                println!("\n  • {} [{}]", job.job_id.bright_yellow(), status_str);
                println!("    Input: {}", &job.input_hash[..16].bright_cyan());
                
                if !job.output_hash.is_empty() {
                    println!("    Output: {}", &job.output_hash[..16].bright_cyan());
                }
                
                if !job.assigned_worker.is_empty() {
                    println!("    Worker: {}", job.assigned_worker);
                }
            }
        }

        Ok(())
    }

    pub async fn scheduler_status(&self) -> Result<()> {
        println!("{}", "📡 Scheduler Configuration".bold());
        println!("   Address: {}", self.config.scheduler.addr.bright_green());
        println!("   CAS Root: {}", self.config.cas.root);
        
        // Try to connect
        let scheduler_addr = format!("http://{}", self.config.scheduler.addr);
        match SchedulerClient::connect(scheduler_addr).await {
            Ok(_) => println!("   Status: {}", "Online ✓".green()),
            Err(_) => println!("   Status: {}", "Offline ✗".red()),
        }

        Ok(())
    }

    pub fn show_help(&self) {
        println!("{}", "Available Commands:".bold().underline());
        println!();
        println!("  {}  {}", "cas put <file>".cyan(), "Store a file in CAS");
        println!("  {}  {}", "cas get <hash> <out>".cyan(), "Retrieve a blob from CAS");
        println!("  {}  {}", "cas exists <hash>".cyan(), "Check if a hash exists in CAS");
        println!("  {}  {}", "cas list".cyan(), "List all hashes in CAS");
        println!();
        println!("  {}  {}", "job submit <hash>".cyan(), "Submit a job with input hash");
        println!("  {}  {}", "job status <id>".cyan(), "Get status of a job");
        println!("  {}  {}", "jobs list [limit]".cyan(), "List recent jobs");
        println!();
        println!("  {}  {}", "workers list".cyan(), "List registered workers");
        println!("  {}  {}", "scheduler status".cyan(), "Show scheduler information");
        println!();
        println!("  {}  {}", "help".cyan(), "Show this help message");
        println!("  {}  {}", "exit/quit".cyan(), "Exit the shell");
    }
}

