use cargo_distbuild::cas::Cas;
use cargo_distbuild::common::Config;
use cargo_distbuild::proto::distbuild::scheduler_client::SchedulerClient;
use cargo_distbuild::proto::distbuild::*;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_cas_basic_operations() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path()).unwrap();

    // Test put
    let data = b"hello world from test";
    let hash = cas.put(data).unwrap();
    assert_eq!(hash.len(), 64); // SHA-256

    // Test exists
    assert!(cas.exists(&hash));

    // Test get
    let retrieved = cas.get(&hash).unwrap();
    assert_eq!(retrieved, data);

    // Test deduplication
    let hash2 = cas.put(data).unwrap();
    assert_eq!(hash, hash2);
}

#[tokio::test]
async fn test_cas_multiple_blobs() {
    let temp_dir = TempDir::new().unwrap();
    let cas = Cas::new(temp_dir.path()).unwrap();

    let blobs = vec![
        b"blob 1".as_ref(),
        b"blob 2".as_ref(),
        b"blob 3".as_ref(),
    ];

    let mut hashes = Vec::new();
    for blob in &blobs {
        let hash = cas.put(blob).unwrap();
        hashes.push(hash);
    }

    // Verify all hashes are different
    assert_eq!(hashes.len(), 3);
    assert_ne!(hashes[0], hashes[1]);
    assert_ne!(hashes[1], hashes[2]);

    // Verify all can be retrieved
    for (blob, hash) in blobs.iter().zip(hashes.iter()) {
        let retrieved = cas.get(hash).unwrap();
        assert_eq!(retrieved, *blob);
    }

    // Test list_all
    let all_hashes = cas.list_all().unwrap();
    assert_eq!(all_hashes.len(), 3);
    for hash in &hashes {
        assert!(all_hashes.contains(hash));
    }
}

#[tokio::test]
async fn test_scheduler_worker_registration() {
    // Create a test config
    let temp_dir = TempDir::new().unwrap();
    let mut config = Config::default();
    config.scheduler.addr = "127.0.0.1:15000".to_string();
    config.cas.root = temp_dir.path().to_str().unwrap().to_string();

    // Start scheduler in background
    let scheduler_addr = config.scheduler.addr.clone();
    tokio::spawn(async move {
        cargo_distbuild::scheduler::run_scheduler(scheduler_addr)
            .await
            .unwrap();
    });

    // Wait for scheduler to start
    sleep(Duration::from_secs(1)).await;

    // Connect as a client and register a worker
    let mut client = SchedulerClient::connect(format!("http://{}", config.scheduler.addr))
        .await
        .unwrap();

    let request = RegisterWorkerRequest {
        worker_id: "test-worker-1".to_string(),
        address: "127.0.0.1:16001".to_string(),
        capacity: 4,
        labels: std::collections::HashMap::new(),
    };

    let response = client.register_worker(request).await.unwrap();
    let resp = response.into_inner();
    
    assert!(resp.success);
    assert!(resp.message.contains("test-worker-1"));

    // List workers
    let list_request = ListWorkersRequest {};
    let list_response = client.list_workers(list_request).await.unwrap();
    let list_resp = list_response.into_inner();

    assert_eq!(list_resp.workers.len(), 1);
    assert_eq!(list_resp.workers[0].worker_id, "test-worker-1");
}

#[tokio::test]
async fn test_job_submission_and_status() {
    // Create a test config
    let temp_dir = TempDir::new().unwrap();
    let mut config = Config::default();
    config.scheduler.addr = "127.0.0.1:15001".to_string();
    config.cas.root = temp_dir.path().to_str().unwrap().to_string();

    // Start scheduler
    let scheduler_addr = config.scheduler.addr.clone();
    tokio::spawn(async move {
        cargo_distbuild::scheduler::run_scheduler(scheduler_addr)
            .await
            .unwrap();
    });

    // Wait for scheduler to start
    sleep(Duration::from_secs(1)).await;

    // Setup CAS and add test data
    let cas = Cas::new(&config.cas.root).unwrap();
    let test_data = b"test input data";
    let input_hash = cas.put(test_data).unwrap();

    // Connect and submit a job
    let mut client = SchedulerClient::connect(format!("http://{}", config.scheduler.addr))
        .await
        .unwrap();

    let job_id = "test-job-123".to_string();
    let submit_request = SubmitJobRequest {
        job_id: job_id.clone(),
        input_hash: input_hash.clone(),
        job_type: "test-transform".to_string(),
        metadata: std::collections::HashMap::new(),
    };

    let submit_response = client.submit_job(submit_request).await.unwrap();
    let submit_resp = submit_response.into_inner();
    
    assert!(submit_resp.success);

    // Check job status
    let status_request = GetJobStatusRequest {
        job_id: job_id.clone(),
    };

    let status_response = client.get_job_status(status_request).await.unwrap();
    let status_resp = status_response.into_inner();

    assert_eq!(status_resp.job_id, job_id);
    // Should be PENDING (0) since no worker picked it up
    assert_eq!(status_resp.status, 0);

    // List jobs
    let list_request = ListJobsRequest { limit: 10 };
    let list_response = client.list_jobs(list_request).await.unwrap();
    let list_resp = list_response.into_inner();

    assert_eq!(list_resp.jobs.len(), 1);
    assert_eq!(list_resp.jobs[0].job_id, job_id);
}

#[tokio::test]
async fn test_end_to_end_workflow() {
    // This test simulates the complete workflow:
    // 1. Start scheduler
    // 2. Start worker
    // 3. Put data in CAS
    // 4. Submit job
    // 5. Worker processes job
    // 6. Retrieve output from CAS

    let temp_dir = TempDir::new().unwrap();
    let mut config = Config::default();
    config.scheduler.addr = "127.0.0.1:15002".to_string();
    config.cas.root = temp_dir.path().to_str().unwrap().to_string();

    // Start scheduler
    let scheduler_config = config.clone();
    let scheduler_addr = scheduler_config.scheduler.addr.clone();
    tokio::spawn(async move {
        cargo_distbuild::scheduler::run_scheduler(scheduler_addr)
            .await
            .unwrap();
    });

    sleep(Duration::from_secs(1)).await;

    // Start worker
    let worker_config = config.clone();
    let cas = Arc::new(Cas::new(&worker_config.cas.root).unwrap());
    let worker_cas = cas.clone();
    tokio::spawn(async move {
        cargo_distbuild::worker::run_worker(
            "test-worker-e2e".to_string(),
            16002,
            worker_config,
            worker_cas,
        )
        .await
        .unwrap();
    });

    sleep(Duration::from_secs(2)).await;

    // Put test data in CAS
    let test_input = b"input data for processing";
    let input_hash = cas.put(test_input).unwrap();

    // Submit job via gRPC
    let mut client = SchedulerClient::connect(format!("http://{}", config.scheduler.addr))
        .await
        .unwrap();

    let job_id = format!("e2e-job-{}", uuid::Uuid::new_v4());
    let submit_request = SubmitJobRequest {
        job_id: job_id.clone(),
        input_hash: input_hash.clone(),
        job_type: "transform".to_string(),
        metadata: std::collections::HashMap::new(),
    };

    let response = client.submit_job(submit_request).await.unwrap();
    assert!(response.into_inner().success);

    // Wait for worker to potentially pick up the job
    sleep(Duration::from_secs(3)).await;

    // Check if job was assigned
    let status_request = GetJobStatusRequest {
        job_id: job_id.clone(),
    };
    let status_response = client.get_job_status(status_request).await.unwrap();
    let status = status_response.into_inner();

    // Job should at least be submitted (status could be PENDING or ASSIGNED)
    assert!(status.status <= 2); // PENDING, ASSIGNED, or RUNNING
}

#[tokio::test]
async fn test_worker_heartbeat() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = Config::default();
    config.scheduler.addr = "127.0.0.1:15003".to_string();
    config.cas.root = temp_dir.path().to_str().unwrap().to_string();

    // Start scheduler
    let scheduler_addr = config.scheduler.addr.clone();
    tokio::spawn(async move {
        cargo_distbuild::scheduler::run_scheduler(scheduler_addr)
            .await
            .unwrap();
    });

    sleep(Duration::from_secs(1)).await;

    // Start worker
    let worker_config = config.clone();
    let cas = Arc::new(Cas::new(&worker_config.cas.root).unwrap());
    tokio::spawn(async move {
        cargo_distbuild::worker::run_worker(
            "test-worker-hb".to_string(),
            16003,
            worker_config,
            cas,
        )
        .await
        .unwrap();
    });

    // Wait for worker to register and send heartbeat
    sleep(Duration::from_secs(3)).await;

    // Check if worker is registered
    let mut client = SchedulerClient::connect(format!("http://{}", config.scheduler.addr))
        .await
        .unwrap();

    let list_request = ListWorkersRequest {};
    let list_response = client.list_workers(list_request).await.unwrap();
    let list_resp = list_response.into_inner();

    // Worker should be registered
    assert!(!list_resp.workers.is_empty());
    
    let worker = list_resp.workers.iter()
        .find(|w| w.worker_id == "test-worker-hb");
    
    assert!(worker.is_some());
    let worker = worker.unwrap();
    
    // Check that heartbeat timestamp is recent (within last 30 seconds)
    let now = chrono::Utc::now().timestamp();
    assert!(now - worker.last_heartbeat < 30);
}
