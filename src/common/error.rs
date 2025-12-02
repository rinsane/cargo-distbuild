use thiserror::Error;

#[derive(Error, Debug)]
pub enum DistbuildError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("CAS error: {0}")]
    Cas(String),

    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Worker not found: {0}")]
    WorkerNotFound(String),

    #[error("Invalid hash: {0}")]
    InvalidHash(String),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, DistbuildError>;

