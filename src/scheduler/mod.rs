use crate::common::types::{JobMetadata, JobStatusEnum, WorkerMetadata};
use crate::proto::distbuild::*;
use crate::proto::distbuild::scheduler_server::{Scheduler, SchedulerServer};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{transport::Server, Request, Response, Status};

pub struct SchedulerService {
    state: Arc<RwLock<SchedulerState>>,
}

#[derive(Default)]
struct SchedulerState {
    workers: HashMap<String, WorkerMetadata>,
    jobs: HashMap<String, JobMetadata>,
}

impl SchedulerService {
    pub fn new() -> Self {
        SchedulerService {
            state: Arc::new(RwLock::new(SchedulerState::default())),
        }
    }

    pub async fn run(self, addr: String) -> Result<()> {
        let addr = addr.parse()?;
        println!("🚀 Scheduler listening on {}", addr);

        Server::builder()
            .add_service(SchedulerServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }

    async fn assign_jobs_to_workers(&self) {
        let now = chrono::Utc::now().timestamp();
        let mut state = self.state.write().await;
        
        // Mark workers as offline if heartbeat is too old (10 seconds)
        let offline_workers: Vec<String> = state
            .workers
            .iter()
            .filter(|(_, worker)| now - worker.last_heartbeat > 10)
            .map(|(id, _)| id.clone())
            .collect();
        
        for worker_id in offline_workers {
            state.workers.remove(&worker_id);
            println!("⚠️  Worker {} marked offline (no heartbeat)", worker_id);
        }
        
        // Find pending jobs
        let pending_jobs: Vec<(String, String, String, String)> = state
            .jobs
            .iter()
            .filter(|(_, job)| job.status == JobStatusEnum::Pending)
            .map(|(id, job)| (id.clone(), job.input_hash.clone(), job.job_type.clone(), job.metadata.clone().into_iter().collect::<Vec<_>>().into_iter().map(|(k,v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(",")))
            .collect();

        // Find available workers (healthy and with capacity)
        let available_workers: Vec<(String, String)> = state
            .workers
            .iter()
            .filter(|(_, worker)| worker.active_jobs < worker.capacity && now - worker.last_heartbeat < 10)
            .map(|(id, worker)| (id.clone(), worker.address.clone()))
            .collect();

        if pending_jobs.is_empty() || available_workers.is_empty() {
            return;
        }

        // Collect assignments to make outside the lock
        let mut assignments = Vec::new();
        
        for ((job_id, input_hash, job_type, _metadata), (worker_id, worker_addr)) in 
            pending_jobs.iter().zip(available_workers.iter()) 
        {
            if let Some(job) = state.jobs.get_mut(job_id) {
                job.status = JobStatusEnum::Assigned;
                job.assigned_worker = Some(worker_id.clone());
                
                assignments.push((
                    job_id.clone(),
                    input_hash.clone(),
                    job_type.clone(),
                    worker_id.clone(),
                    worker_addr.clone(),
                ));
            }
            if let Some(worker) = state.workers.get_mut(worker_id) {
                worker.active_jobs += 1;
            }
        }
        
        // Drop lock before async operations
        drop(state);
        
        // Execute jobs on workers
        for (job_id, input_hash, job_type, worker_id, worker_addr) in assignments {
            let self_clone = SchedulerService {
                state: self.state.clone(),
            };
            
            tokio::spawn(async move {
                if let Err(e) = self_clone.dispatch_job_to_worker(
                    &job_id,
                    &input_hash,
                    &job_type,
                    &worker_id,
                    &worker_addr,
                ).await {
                    eprintln!("❌ Failed to dispatch job {} to {}: {}", job_id, worker_id, e);
                    
                    // Mark job as failed
                    let mut state = self_clone.state.write().await;
                    if let Some(job) = state.jobs.get_mut(&job_id) {
                        job.status = JobStatusEnum::Failed;
                        job.completed_at = Some(chrono::Utc::now().timestamp());
                    }
                    if let Some(worker) = state.workers.get_mut(&worker_id) {
                        worker.active_jobs = worker.active_jobs.saturating_sub(1);
                    }
                }
            });
        }
    }
    
    async fn dispatch_job_to_worker(
        &self,
        job_id: &str,
        input_hash: &str,
        job_type: &str,
        worker_id: &str,
        worker_addr: &str,
    ) -> Result<()> {
        use crate::proto::distbuild::worker_client::WorkerClient;
        
        println!("📤 Dispatching job {} to worker {} at {}", job_id, worker_id, worker_addr);
        
        // Update job status to RUNNING
        {
            let mut state = self.state.write().await;
            if let Some(job) = state.jobs.get_mut(job_id) {
                job.status = JobStatusEnum::Running;
            }
        }
        
        // Connect to worker and execute job
        let worker_url = format!("http://{}", worker_addr);
        let mut client = WorkerClient::connect(worker_url).await?;
        
        let request = ExecuteJobRequest {
            job_id: job_id.to_string(),
            input_hash: input_hash.to_string(),
            job_type: job_type.to_string(),
            metadata: std::collections::HashMap::new(),
        };
        
        let _response = client.execute_job(request).await?;
        
        Ok(())
    }
}

#[tonic::async_trait]
impl Scheduler for SchedulerService {
    async fn register_worker(
        &self,
        request: Request<RegisterWorkerRequest>,
    ) -> Result<Response<RegisterWorkerResponse>, Status> {
        let req = request.into_inner();
        let worker_id = req.worker_id.clone();

        let worker = WorkerMetadata {
            worker_id: worker_id.clone(),
            address: req.address,
            capacity: req.capacity,
            active_jobs: 0,
            last_heartbeat: chrono::Utc::now().timestamp(),
            labels: req.labels,
        };

        let mut state = self.state.write().await;
        state.workers.insert(worker_id.clone(), worker);

        println!("✅ Worker registered: {}", worker_id);

        Ok(Response::new(RegisterWorkerResponse {
            success: true,
            message: format!("Worker {} registered successfully", worker_id),
        }))
    }

    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let req = request.into_inner();
        let worker_id = req.worker_id.clone();

        let mut state = self.state.write().await;
        
        if let Some(worker) = state.workers.get_mut(&worker_id) {
            worker.last_heartbeat = chrono::Utc::now().timestamp();
            worker.active_jobs = req.active_jobs;
        } else {
            return Err(Status::not_found(format!("Worker {} not found", worker_id)));
        }

        Ok(Response::new(HeartbeatResponse {
            success: true,
            jobs_to_execute: vec![], // No longer used - scheduler calls ExecuteJob directly
        }))
    }

    async fn submit_job(
        &self,
        request: Request<SubmitJobRequest>,
    ) -> Result<Response<SubmitJobResponse>, Status> {
        let req = request.into_inner();
        let job_id = req.job_id.clone();

        let job = JobMetadata {
            job_id: job_id.clone(),
            input_hash: req.input_hash,
            output_hash: None,
            job_type: req.job_type,
            status: JobStatusEnum::Pending,
            assigned_worker: None,
            submitted_at: chrono::Utc::now().timestamp(),
            completed_at: None,
            metadata: req.metadata,
        };

        let mut state = self.state.write().await;
        state.jobs.insert(job_id.clone(), job);

        println!("📋 Job submitted: {}", job_id);

        // Drop the lock before async work
        drop(state);

        // Try to assign jobs
        self.assign_jobs_to_workers().await;

        Ok(Response::new(SubmitJobResponse {
            success: true,
            job_id,
            message: "Job submitted successfully".to_string(),
        }))
    }

    async fn get_job_status(
        &self,
        request: Request<GetJobStatusRequest>,
    ) -> Result<Response<GetJobStatusResponse>, Status> {
        let req = request.into_inner();
        let job_id = req.job_id;

        let state = self.state.read().await;
        
        if let Some(job) = state.jobs.get(&job_id) {
            Ok(Response::new(GetJobStatusResponse {
                job_id: job.job_id.clone(),
                status: job.status.into(),
                output_hash: job.output_hash.clone().unwrap_or_default(),
                error: String::new(),
                assigned_worker: job.assigned_worker.clone().unwrap_or_default(),
            }))
        } else {
            Err(Status::not_found(format!("Job {} not found", job_id)))
        }
    }

    async fn list_workers(
        &self,
        _request: Request<ListWorkersRequest>,
    ) -> Result<Response<ListWorkersResponse>, Status> {
        let now = chrono::Utc::now().timestamp();
        let mut state = self.state.write().await;
        
        // Remove offline workers (no heartbeat for 10+ seconds)
        let offline_workers: Vec<String> = state
            .workers
            .iter()
            .filter(|(_, worker)| now - worker.last_heartbeat > 10)
            .map(|(id, _)| id.clone())
            .collect();
        
        for worker_id in &offline_workers {
            state.workers.remove(worker_id);
            println!("⚠️  Worker {} removed (offline for >10s)", worker_id);
        }
        
        let workers = state
            .workers
            .values()
            .map(|w| WorkerInfo {
                worker_id: w.worker_id.clone(),
                address: w.address.clone(),
                capacity: w.capacity,
                active_jobs: w.active_jobs,
                last_heartbeat: w.last_heartbeat,
                labels: w.labels.clone(),
            })
            .collect();

        Ok(Response::new(ListWorkersResponse { workers }))
    }

    async fn list_jobs(
        &self,
        request: Request<ListJobsRequest>,
    ) -> Result<Response<ListJobsResponse>, Status> {
        let req = request.into_inner();
        let state = self.state.read().await;
        
        let mut jobs: Vec<JobInfo> = state
            .jobs
            .values()
            .map(|j| JobInfo {
                job_id: j.job_id.clone(),
                status: j.status.into(),
                input_hash: j.input_hash.clone(),
                output_hash: j.output_hash.clone().unwrap_or_default(),
                assigned_worker: j.assigned_worker.clone().unwrap_or_default(),
                submitted_at: j.submitted_at,
                completed_at: j.completed_at.unwrap_or(0),
            })
            .collect();

        // Sort by submission time (newest first)
        jobs.sort_by(|a, b| b.submitted_at.cmp(&a.submitted_at));

        // Apply limit
        if req.limit > 0 {
            jobs.truncate(req.limit as usize);
        }

        Ok(Response::new(ListJobsResponse { jobs }))
    }

    async fn report_job_result(
        &self,
        request: Request<ReportJobResultRequest>,
    ) -> Result<Response<ReportJobResultResponse>, Status> {
        let req = request.into_inner();
        let job_id = req.job_id.clone();

        let mut state = self.state.write().await;
        
        // Get the assigned worker_id before mutable borrows
        let worker_id = state.jobs.get(&job_id)
            .and_then(|job| job.assigned_worker.clone());
        
        if let Some(job) = state.jobs.get_mut(&job_id) {
            if req.success {
                let output_hash = req.output_hash.clone();
                job.status = JobStatusEnum::Completed;
                job.output_hash = Some(req.output_hash);
                job.completed_at = Some(chrono::Utc::now().timestamp());
                
                println!("✅ Job completed: {} (output: {})", job_id, output_hash);
            } else {
                let error = req.error.clone();
                job.status = JobStatusEnum::Failed;
                job.completed_at = Some(chrono::Utc::now().timestamp());
                
                println!("❌ Job failed: {} (error: {})", job_id, error);
            }
        } else {
            return Err(Status::not_found(format!("Job {} not found", job_id)));
        }
        
        // Decrease worker's active job count (after job borrow is released)
        if let Some(worker_id) = worker_id {
            if let Some(worker) = state.workers.get_mut(&worker_id) {
                worker.active_jobs = worker.active_jobs.saturating_sub(1);
            }
        }

        Ok(Response::new(ReportJobResultResponse {
            acknowledged: true,
        }))
    }
}

pub async fn run_scheduler(addr: String) -> Result<()> {
    let service = SchedulerService::new();
    service.run(addr).await
}

