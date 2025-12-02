# Critical Bug Fixes - Iteration 2

**Date**: December 2, 2025

## Issues Found During Testing

### 🐛 Bug #1: Dead Workers Still Show as Alive

**Problem**: When workers are stopped (Ctrl+C), the scheduler still lists them as alive. The `workers list` command shows them with old heartbeat timestamps, but doesn't mark them as offline.

**Root Cause**: No timeout detection in the scheduler

**Fix**: 
1. Added 30-second timeout check in `list_workers()`
2. Added timeout check in `assign_jobs_to_workers()`
3. Automatically remove workers with heartbeat older than 30 seconds

```rust
// Check if worker is offline (no heartbeat for 30+ seconds)
let now = chrono::Utc::now().timestamp();
if now - worker.last_heartbeat > 30 {
    state.workers.remove(&worker_id);
    println!("⚠️  Worker {} removed (offline for >30s)", worker_id);
}
```

**Result**: 
- Dead workers are automatically removed from the pool
- Jobs won't be assigned to offline workers
- `workers list` only shows healthy workers

---

### 🐛 Bug #2: Jobs Never Complete

**Problem**: Worker prints "📋 Received 1 jobs to execute" but never actually executes the job. No completion message.

**Root Cause**: The heartbeat-based job assignment was a stub - workers weren't actually calling the ExecuteJob RPC.

**Fix**: Changed architecture to **push-based**:
1. Scheduler now directly calls `ExecuteJob` RPC on workers
2. Worker immediately starts execution when RPC is received
3. Worker reports completion via `ReportJobResult` RPC

```rust
async fn dispatch_job_to_worker(&self, job_id, input_hash, job_type, worker_id, worker_addr) {
    let mut client = WorkerClient::connect(worker_url).await?;
    let request = ExecuteJobRequest { job_id, input_hash, job_type, ... };
    client.execute_job(request).await?; // Direct RPC call
}
```

**Result**:
- Jobs execute immediately when assigned
- Workers report completion back to scheduler
- Job status properly transitions: PENDING → ASSIGNED → RUNNING → COMPLETED/FAILED

---

### 🐛 Bug #3: No Validation or Error Reporting

**Problem**: Submitting a README.md file doesn't fail even though it's not compilable Rust code. Worker should detect this and report an error.

**Root Cause**: Worker's dummy transformation didn't validate input

**Fix**: Added basic Rust code validation:
```rust
// Check if input looks like Rust code
if !input_str.contains("fn ") && !input_str.contains("pub ") && !input_str.contains("use ") {
    anyhow::bail!(
        "Input doesn't appear to be valid Rust source code. \
        Expected Rust syntax (fn, pub, use, etc.)"
    );
}
```

Worker now:
1. Validates that input contains Rust keywords
2. Reports errors via `ReportJobResult` with success=false
3. Scheduler marks job as FAILED
4. Master can see failure in `job status` and `jobs list`

**Result**:
- Invalid inputs are rejected
- Errors are properly reported
- Jobs marked as FAILED in scheduler
- Clear error messages for debugging

---

## Architecture Changes

### Before (Broken)
```
Master submits job
    ↓
Scheduler assigns to worker (just marks it)
    ↓
Worker heartbeat receives job ID
    ↓
Worker... does nothing (stub code)
    ↓
Job stays in ASSIGNED forever
```

### After (Fixed)
```
Master submits job
    ↓
Scheduler finds available worker
    ↓
Scheduler calls ExecuteJob RPC on worker
    ↓
Worker validates input
    ↓
Worker executes (or fails with error)
    ↓
Worker calls ReportJobResult RPC to scheduler
    ↓
Job marked COMPLETED or FAILED
```

---

## Testing the Fixes

### Test 1: Worker Offline Detection

```bash
# Terminal 1
cargo run --release -- scheduler run

# Terminal 2
cargo run --release -- worker run --id worker-1 --port 6001

# Terminal 3
cargo run --release
cargo-distbuild> workers list
# Shows worker-1

# Go to Terminal 2, press Ctrl+C (stop worker)

# Back to Terminal 3
cargo-distbuild> workers list
# Wait 30+ seconds, run again
cargo-distbuild> workers list
# worker-1 should be gone!
```

**Expected**: Worker disappears from list after 30 seconds

---

### Test 2: Job Completion

```bash
# Start scheduler + worker (same as above)

# Terminal 3
cargo-distbuild> echo "fn main() { println!(\"test\"); }" > /tmp/test.rs
cargo-distbuild> cas put /tmp/test.rs
# Note the hash

cargo-distbuild> job submit <hash>
cargo-distbuild> jobs list
# Should show COMPLETED after a moment
```

**Expected**: Job completes and shows COMPLETED status

---

### Test 3: Error Reporting (README should fail)

```bash
# Start scheduler + worker

cargo-distbuild> cas put README.md
# Note the hash

cargo-distbuild> job submit <hash>
cargo-distbuild> jobs list
# Should show FAILED

cargo-distbuild> job status <job-id>
# Should show error message about invalid Rust code
```

**Expected**: Job fails with clear error message

---

## Files Modified

1. `src/scheduler/mod.rs`:
   - Added worker timeout detection (30s)
   - Changed from pull-based to push-based job dispatch
   - Scheduler now directly calls ExecuteJob on workers
   - Remove dead workers automatically

2. `src/worker/mod.rs`:
   - Added input validation (checks for Rust syntax)
   - Better error messages
   - Proper error reporting to scheduler

3. `src/proto/distbuild.proto`:
   - Added ReportJobResult RPC (already done in previous fix)

---

## Next Steps

With these fixes, the system is now ready for:
1. Real Rust compilation (replace dummy transformation with rustc)
2. Cargo wrapper implementation
3. Test workspace compilation

---

## Performance Characteristics

- **Worker timeout**: 30 seconds (configurable)
- **Heartbeat interval**: 10 seconds (from config.toml)
- **Job dispatch**: Immediate (push-based)
- **Error detection**: Real-time

---

## Known Limitations

1. **No job retry**: If a worker dies mid-job, the job is lost
2. **No job timeout**: Jobs can run forever
3. **Basic validation**: Only checks for Rust keywords, not actual syntax
4. **No partial failure**: All-or-nothing job execution

These will be addressed in future iterations as we add fault tolerance.

