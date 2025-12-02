use crate::common::Config;
use crate::master::commands::CommandExecutor;
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cargo-distbuild")]
#[command(about = "Distributed Rust build system", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// CAS operations
    Cas {
        #[command(subcommand)]
        action: CasCommands,
    },
    
    /// Scheduler operations
    Scheduler {
        #[command(subcommand)]
        action: SchedulerCommands,
    },
    
    /// Worker operations
    Worker {
        #[command(subcommand)]
        action: WorkerCommands,
    },
    
    /// Master operations
    Master {
        #[command(subcommand)]
        action: MasterCommands,
    },
}

#[derive(Subcommand)]
pub enum CasCommands {
    /// Store a file in CAS
    Put {
        /// Path to the file to store
        file: String,
    },
    
    /// Retrieve a blob from CAS
    Get {
        /// Hash of the blob
        hash: String,
        /// Output file path
        output: String,
    },
    
    /// Check if a hash exists
    Exists {
        /// Hash to check
        hash: String,
    },
    
    /// List all blobs in CAS
    List,
}

#[derive(Subcommand)]
pub enum SchedulerCommands {
    /// Run the scheduler
    Run {
        /// Address to bind to (default: from config)
        #[arg(long)]
        addr: Option<String>,
    },
    
    /// Show scheduler status
    Status,
}

#[derive(Subcommand)]
pub enum WorkerCommands {
    /// Run a worker
    Run {
        /// Worker ID
        #[arg(long, default_value = "worker-1")]
        id: String,
        
        /// Port to listen on
        #[arg(long, default_value = "6001")]
        port: u16,
    },
}

#[derive(Subcommand)]
pub enum MasterCommands {
    /// Submit a job
    SubmitJob {
        /// Input hash from CAS
        input_hash: String,
    },
    
    /// Get job status
    JobStatus {
        /// Job ID
        job_id: String,
    },
    
    /// List jobs
    ListJobs {
        /// Maximum number of jobs to show
        #[arg(long, default_value = "10")]
        limit: u32,
    },
    
    /// List workers
    ListWorkers,
}

pub async fn run_cli(cli: Cli) -> Result<()> {
    let config = Config::load_default()?;

    match cli.command {
        Some(Commands::Cas { action }) => {
            let executor = CommandExecutor::new(config)?;
            
            match action {
                CasCommands::Put { file } => {
                    executor.cas_put(&file).await?;
                }
                CasCommands::Get { hash, output } => {
                    executor.cas_get(&hash, &output).await?;
                }
                CasCommands::Exists { hash } => {
                    executor.cas_exists(&hash).await?;
                }
                CasCommands::List => {
                    executor.cas_list().await?;
                }
            }
        }
        
        Some(Commands::Scheduler { action }) => {
            match action {
                SchedulerCommands::Run { addr } => {
                    let scheduler_addr = addr.unwrap_or(config.scheduler.addr);
                    crate::scheduler::run_scheduler(scheduler_addr).await?;
                }
                SchedulerCommands::Status => {
                    let executor = CommandExecutor::new(config)?;
                    executor.scheduler_status().await?;
                }
            }
        }
        
        Some(Commands::Worker { action }) => {
            match action {
                WorkerCommands::Run { id, port } => {
                    let cas = std::sync::Arc::new(crate::cas::Cas::new(&config.cas.root)?);
                    crate::worker::run_worker(id, port, config, cas).await?;
                }
            }
        }
        
        Some(Commands::Master { action }) => {
            let executor = CommandExecutor::new(config)?;
            
            match action {
                MasterCommands::SubmitJob { input_hash } => {
                    executor.submit_job(&input_hash).await?;
                }
                MasterCommands::JobStatus { job_id } => {
                    executor.job_status(&job_id).await?;
                }
                MasterCommands::ListJobs { limit } => {
                    executor.list_jobs(limit).await?;
                }
                MasterCommands::ListWorkers => {
                    executor.list_workers().await?;
                }
            }
        }
        
        None => {
            // No command provided - start interactive REPL
            crate::master::repl::run_repl().await?;
        }
    }

    Ok(())
}

