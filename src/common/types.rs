use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMetadata {
    pub job_id: String,
    pub input_hash: String,
    pub output_hash: Option<String>,
    pub job_type: String,
    pub status: JobStatusEnum,
    pub assigned_worker: Option<String>,
    pub submitted_at: i64,
    pub completed_at: Option<i64>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatusEnum {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed,
}

impl From<i32> for JobStatusEnum {
    fn from(value: i32) -> Self {
        match value {
            0 => JobStatusEnum::Pending,
            1 => JobStatusEnum::Assigned,
            2 => JobStatusEnum::Running,
            3 => JobStatusEnum::Completed,
            4 => JobStatusEnum::Failed,
            _ => JobStatusEnum::Failed,
        }
    }
}

impl From<JobStatusEnum> for i32 {
    fn from(status: JobStatusEnum) -> Self {
        match status {
            JobStatusEnum::Pending => 0,
            JobStatusEnum::Assigned => 1,
            JobStatusEnum::Running => 2,
            JobStatusEnum::Completed => 3,
            JobStatusEnum::Failed => 4,
        }
    }
}

impl std::fmt::Display for JobStatusEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatusEnum::Pending => write!(f, "PENDING"),
            JobStatusEnum::Assigned => write!(f, "ASSIGNED"),
            JobStatusEnum::Running => write!(f, "RUNNING"),
            JobStatusEnum::Completed => write!(f, "COMPLETED"),
            JobStatusEnum::Failed => write!(f, "FAILED"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMetadata {
    pub worker_id: String,
    pub address: String,
    pub capacity: u32,
    pub active_jobs: u32,
    pub last_heartbeat: i64,
    pub labels: HashMap<String, String>,
}

