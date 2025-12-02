use crate::cas::Cas;
use crate::common::Config;
use crate::proto::distbuild::*;
use crate::proto::distbuild::scheduler_client::SchedulerClient;
use crate::proto::distbuild::worker_server::{Worker, WorkerServer};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tonic::{transport::Server, Request, Response, Status};

pub struct WorkerService {
    worker_id: String,
    address: String,
    capacity: u32,
    cas: Arc<Cas>,
    scheduler_addr: String,
    state: Arc<RwLock<WorkerState>>,
}

#[derive(Default)]
struct WorkerState {
    active_jobs: HashMap<String, JobInfo>,
}

#[derive(Debug, Clone)]
struct JobInfo {
    job_id: String,
    status: String,
}

impl WorkerService {
    pub fn new(worker_id: String, address: String, config: Config, cas: Arc<Cas>) -> Self {
        WorkerService {
            worker_id,
            address,
            capacity: config.worker.capacity,
            cas,
            scheduler_addr: format!("http://{}", config.scheduler.addr),
            state: Arc::new(RwLock::new(WorkerState::default())),
        }
    }

    /// Run the worker (gRPC server + heartbeat loop)
    pub async fn run(self) -> Result<()> {
        let worker_id = self.worker_id.clone();
        let address = self.address.clone();
        
        // Start heartbeat loop
        let heartbeat_worker = self.clone_for_heartbeat();
        tokio::spawn(async move {
            if let Err(e) = heartbeat_worker.heartbeat_loop().await {
                eprintln!("❌ Heartbeat loop error: {}", e);
            }
        });

        // Register with scheduler
        self.register().await?;

        // Start gRPC server
        let addr = address.parse()?;
        println!("🔧 Worker {} listening on {}", worker_id, addr);

        Server::builder()
            .add_service(WorkerServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }

    fn clone_for_heartbeat(&self) -> Self {
        WorkerService {
            worker_id: self.worker_id.clone(),
            address: self.address.clone(),
            capacity: self.capacity,
            cas: self.cas.clone(),
            scheduler_addr: self.scheduler_addr.clone(),
            state: self.state.clone(),
        }
    }

    async fn register(&self) -> Result<()> {
        let mut client = SchedulerClient::connect(self.scheduler_addr.clone())
            .await
            .context("Failed to connect to scheduler")?;

        let request = RegisterWorkerRequest {
            worker_id: self.worker_id.clone(),
            address: self.address.clone(),
            capacity: self.capacity,
            labels: HashMap::new(),
        };

        let response = client.register_worker(request).await?;
        let resp = response.into_inner();

        if resp.success {
            println!("✅ Registered with scheduler: {}", resp.message);
        } else {
            anyhow::bail!("Failed to register: {}", resp.message);
        }

        Ok(())
    }

    async fn heartbeat_loop(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(10));

        loop {
            interval.tick().await;

            if let Err(e) = self.send_heartbeat().await {
                eprintln!("❌ Heartbeat failed: {}", e);
            }
        }
    }

    async fn send_heartbeat(&self) -> Result<()> {
        let mut client = SchedulerClient::connect(self.scheduler_addr.clone()).await?;

        let state = self.state.read().await;
        let active_jobs = state.active_jobs.len() as u32;
        let available_slots = self.capacity.saturating_sub(active_jobs);

        let request = HeartbeatRequest {
            worker_id: self.worker_id.clone(),
            active_jobs,
            available_slots,
        };

        let response = client.heartbeat(request).await?;
        let resp = response.into_inner();

        if !resp.jobs_to_execute.is_empty() {
            println!("📋 Received {} jobs to execute", resp.jobs_to_execute.len());
            
            // Execute jobs asynchronously
            for job_id in resp.jobs_to_execute {
                let worker = self.clone_for_heartbeat();
                tokio::spawn(async move {
                    if let Err(e) = worker.execute_job_by_id(&job_id).await {
                        eprintln!("❌ Job {} execution failed: {}", job_id, e);
                    }
                });
            }
        }

        Ok(())
    }

    async fn execute_job_by_id(&self, _job_id: &str) -> Result<()> {
        // This path is no longer used - jobs come via gRPC ExecuteJob RPC
        Ok(())
    }
    
    async fn report_completion(&self, job_id: &str, success: bool, output_hash: String, error: String) -> Result<()> {
        let mut client = SchedulerClient::connect(self.scheduler_addr.clone()).await?;
        
        let request = ReportJobResultRequest {
            job_id: job_id.to_string(),
            success,
            output_hash,
            error,
        };
        
        client.report_job_result(request).await?;
        Ok(())
    }

    async fn execute_job_impl(
        &self,
        job_id: &str,
        input_hash: &str,
        job_type: &str,
    ) -> Result<String> {
        println!("🔨 Worker {} executing job: {}", self.worker_id, job_id);
        println!("   Job type: {}", job_type);
        println!("   Input hash: {}", input_hash);

        // Fetch input from CAS
        let input_data = self.cas.get(input_hash)
            .context("Failed to get input from CAS")?;

        println!("   Read {} bytes from CAS", input_data.len());

        // Check if this looks like Rust source code (basic validation)
        let input_str = String::from_utf8_lossy(&input_data);
        
        // For now, simulate compilation validation
        // Real implementation will extract .rs files and run rustc
        if !input_str.contains("fn ") && !input_str.contains("pub ") && !input_str.contains("use ") {
            // Doesn't look like Rust code
            anyhow::bail!(
                "Input doesn't appear to be valid Rust source code. \
                Expected Rust syntax (fn, pub, use, etc.) but found: {}",
                &input_str.chars().take(100).collect::<String>()
            );
        }

        // Dummy transformation: append " + compiled by worker"
        // In real implementation, this would be: rustc <args> -> .rlib output
        let output = format!("{} + compiled by worker {}", input_str, self.worker_id);
        let output_bytes = output.as_bytes();

        // Write output to CAS
        let output_hash = self.cas.put(output_bytes)
            .context("Failed to put output to CAS")?;

        println!("   Output hash: {}", output_hash);
        println!("✅ Job completed successfully");

        Ok(output_hash)
    }
}

#[tonic::async_trait]
impl Worker for WorkerService {
    async fn execute_job(
        &self,
        request: Request<ExecuteJobRequest>,
    ) -> Result<Response<ExecuteJobResponse>, Status> {
        let req = request.into_inner();
        let job_id = req.job_id.clone();

        // Add to active jobs
        {
            let mut state = self.state.write().await;
            state.active_jobs.insert(
                job_id.clone(),
                JobInfo {
                    job_id: job_id.clone(),
                    status: "running".to_string(),
                },
            );
        }

        // Execute the job
        let result = self
            .execute_job_impl(&req.job_id, &req.input_hash, &req.job_type)
            .await;

        // Remove from active jobs
        {
            let mut state = self.state.write().await;
            state.active_jobs.remove(&job_id);
        }

        // Report result to scheduler
        match &result {
            Ok(output_hash) => {
                let _ = self.report_completion(&job_id, true, output_hash.clone(), String::new()).await;
                Ok(Response::new(ExecuteJobResponse {
                    success: true,
                    output_hash: output_hash.clone(),
                    error: String::new(),
                    stdout: String::new(),
                    stderr: String::new(),
                }))
            }
            Err(e) => {
                let error_msg = format!("{:?}", e);
                let _ = self.report_completion(&job_id, false, String::new(), error_msg.clone()).await;
                Ok(Response::new(ExecuteJobResponse {
                    success: false,
                    output_hash: String::new(),
                    error: error_msg,
                    stdout: String::new(),
                    stderr: String::new(),
                }))
            }
        }
    }

    async fn get_status(
        &self,
        _request: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        let state = self.state.read().await;
        let active_jobs = state.active_jobs.len() as u32;

        Ok(Response::new(GetStatusResponse {
            worker_id: self.worker_id.clone(),
            active_jobs,
            capacity: self.capacity,
            healthy: true,
        }))
    }
}

pub async fn run_worker(worker_id: String, port: u16, config: Config, cas: Arc<Cas>) -> Result<()> {
    let address = format!("127.0.0.1:{}", port);
    let service = WorkerService::new(worker_id, address, config, cas);
    service.run().await
}

