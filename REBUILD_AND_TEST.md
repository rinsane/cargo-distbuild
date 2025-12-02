# How to Rebuild and Test After Bug Fixes

## Step 1: Rebuild

```bash
cd /mnt/Extra/COde_work/Things/cargo-distbuild
cargo build --release
```

**Expected**: Clean build with no errors

## Step 2: Start the System

### Terminal 1 - Scheduler
```bash
./target/release/cargo-distbuild scheduler run
```

### Terminal 2 - Worker 1
```bash
./target/release/cargo-distbuild worker run --id worker-1 --port 6001
```

### Terminal 3 - Worker 2
```bash
./target/release/cargo-distbuild worker run --id worker-2 --port 6002
```

### Terminal 4 - Interactive CLI
```bash
./target/release/cargo-distbuild
```

## Step 3: Test the Fixes

### Test 1: Worker Offline Detection ⏱️

In Terminal 4 (CLI):
```bash
cargo-distbuild> workers list
# Should show 2 workers
```

Now **stop Worker 2** (go to Terminal 3, press Ctrl+C)

Wait 30-40 seconds, then:
```bash
cargo-distbuild> workers list
# Should only show worker-1 now!
```

✅ **Pass**: Dead worker is removed after timeout

---

### Test 2: Job Execution with Valid Rust Code ✅

Create a simple Rust file:
```bash
# In another terminal
echo 'fn main() { println!("Hello"); }' > /tmp/hello.rs
```

In Terminal 4 (CLI):
```bash
cargo-distbuild> cas put /tmp/hello.rs
# Note the hash (e.g., abc123...)

cargo-distbuild> job submit abc123...
# Note the job ID

# Wait 2-3 seconds
cargo-distbuild> jobs list
# Should show COMPLETED

cargo-distbuild> job status <job-id>
# Should show status: COMPLETED with output hash
```

✅ **Pass**: Job completes successfully

---

### Test 3: Job Failure with Invalid Input ❌

In Terminal 4 (CLI):
```bash
cargo-distbuild> cas put README.md
# Note the hash

cargo-distbuild> job submit <hash>

# Wait 2-3 seconds
cargo-distbuild> jobs list
# Should show FAILED

cargo-distbuild> job status <job-id>
# Should show error message about "doesn't appear to be valid Rust source code"
```

✅ **Pass**: Job fails with clear error message

---

## What You Should See

### On Scheduler (Terminal 1):
```
🚀 Scheduler listening on 127.0.0.1:5000
✅ Worker registered: worker-1
✅ Worker registered: worker-2
📋 Job submitted: <job-id>
📤 Dispatching job <job-id> to worker worker-1 at 127.0.0.1:6001
✅ Job completed: <job-id> (output: <hash>)
⚠️  Worker worker-2 removed (offline for >30s)
```

### On Worker 1 (Terminal 2):
```
✅ Registered with scheduler: Worker worker-1 registered successfully
🔧 Worker worker-1 listening on 127.0.0.1:6001
🔨 Worker worker-1 executing job: <job-id>
   Job type: transform
   Input hash: abc123...
   Read 42 bytes from CAS
   Output hash: def456...
✅ Job completed successfully
```

### On Worker 2 (Terminal 3):
```
✅ Registered with scheduler: Worker worker-2 registered successfully
🔧 Worker worker-2 listening on 127.0.0.1:6002
^C (you pressed Ctrl+C to stop it)
```

---

## Common Issues

### Issue: "Failed to connect to scheduler"
**Solution**: Make sure Terminal 1 (scheduler) is running first

### Issue: Workers don't appear in `workers list`
**Solution**: Check that workers successfully registered (see "✅ Registered" message)

### Issue: Jobs stay in PENDING
**Solution**: Make sure at least one worker is running and healthy

### Issue: "Hash not found in CAS"
**Solution**: Make sure you `cas put` the file before submitting the job

---

## Next: Real Compilation

Once all tests pass, we're ready to:
1. Implement the Cargo wrapper
2. Replace dummy transformation with real `rustc` execution
3. Test with the `test-workspace/`

See `markdowns/iteration_2/CARGO_WRAPPER_DESIGN.md` (coming next)

