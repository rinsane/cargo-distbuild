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
        let mut state = self.state.write().await;
        
        // Find pending jobs
        let pending_jobs: Vec<String> = state
            .jobs
            .iter()
            .filter(|(_, job)| job.status == JobStatusEnum::Pending)
            .map(|(id, _)| id.clone())
            .collect();

        // Find available workers
        let available_workers: Vec<String> = state
            .workers
            .iter()
            .filter(|(_, worker)| worker.active_jobs < worker.capacity)
            .map(|(id, _)| id.clone())
            .collect();

        // Assign jobs to workers (simple round-robin for now)
        for (job_id, worker_id) in pending_jobs.iter().zip(available_workers.iter()) {
            if let Some(job) = state.jobs.get_mut(job_id) {
                job.status = JobStatusEnum::Assigned;
                job.assigned_worker = Some(worker_id.clone());
            }
            if let Some(worker) = state.workers.get_mut(worker_id) {
                worker.active_jobs += 1;
            }
        }
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

        // Find jobs assigned to this worker
        let jobs_to_execute: Vec<String> = state
            .jobs
            .iter()
            .filter(|(_, job)| {
                job.assigned_worker.as_ref() == Some(&worker_id)
                    && job.status == JobStatusEnum::Assigned
            })
            .map(|(id, _)| id.clone())
            .collect();

        Ok(Response::new(HeartbeatResponse {
            success: true,
            jobs_to_execute,
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
        let state = self.state.read().await;
        
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
}

pub async fn run_scheduler(addr: String) -> Result<()> {
    let service = SchedulerService::new();
    service.run(addr).await
}

