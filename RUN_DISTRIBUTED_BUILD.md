# Running Distributed Compilation on test-workspace

## Step 1: Build Everything

```bash
cd /mnt/Extra/COde_work/Things/cargo-distbuild
cargo build --release
```

This builds:
- `target/release/cargo-distbuild` - Main CLI
- `target/release/cargo-distbuild-wrapper` - The wrapper that intercepts rustc

## Step 2: Start the Distributed System

### Terminal 1 - Scheduler
```bash
cd /mnt/Extra/COde_work/Things/cargo-distbuild
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

## Step 3: Run Distributed Compilation!

### Terminal 4 - Build test-workspace
```bash
cd /mnt/Extra/COde_work/Things/cargo-distbuild/test-workspace

# Set the wrapper environment variable
export RUSTC_WORKSPACE_WRAPPER=$(pwd)/../target/release/cargo-distbuild-wrapper

# Clean previous builds
cargo clean

# BUILD WITH DISTRIBUTED SYSTEM!
cargo build
```

## What You'll See

### On Scheduler (Terminal 1):
```
📋 Job submitted: <job-id-1>
📤 Dispatching job <job-id-1> to worker worker-1
✅ Job completed: <job-id-1>
📋 Job submitted: <job-id-2>
📤 Dispatching job <job-id-2> to worker worker-2
✅ Job completed: <job-id-2>
...
```

### On Workers (Terminals 2 & 3):
```
🔨 Worker worker-1 executing job: <job-id>
   Job type: rust-compile
   Input hash: abc123...
   Read 2048 bytes from CAS
   Output hash: def456...
✅ Job completed successfully
```

### During cargo build (Terminal 4):
```
   Compiling lib-common v0.1.0
🚀 [cargo-distbuild] Intercepted rustc call for crate: "lib_common"
📦 [cargo-distbuild] Packaging source files for CAS...
📤 [cargo-distbuild] Submitting job to scheduler...
⏳ [cargo-distbuild] Waiting for compilation...
📥 [cargo-distbuild] Downloading output...
✅ [cargo-distbuild] Distributed compilation successful

   Compiling lib-math v0.1.0
   Compiling lib-utils v0.1.0  <-- PARALLEL!
🚀 [cargo-distbuild] Intercepted rustc call for crate: "lib_math"
🚀 [cargo-distbuild] Intercepted rustc call for crate: "lib_utils"
...
```

## Expected Behavior

1. **lib-common** compiles first (no dependencies)
2. **lib-math** and **lib-utils** compile IN PARALLEL (both depend only on lib-common)
3. **lib-parser** compiles (depends on lib-utils)
4. **lib-advanced** compiles (depends on lib-math, lib-parser)
5. **main-binary** compiles last (depends on everything)

With 2 workers, you should see stages 2, 4, etc. happening in parallel!

## Verification

After successful build:
```bash
# Run the binary
cd test-workspace
cargo run --bin main-binary

# Should print:
# Welcome to TestApp v1.0.0
# [TestApp] 5 + 3 = 8
# ...
```

## Troubleshooting

### "Failed to connect to scheduler"
- Make sure Terminal 1 (scheduler) is running
- Check config.toml has correct scheduler address

### "Hash not found in CAS"
- CAS might not be shared properly
- Check that all components use same CAS root in config.toml

### Fallback to local compilation
- If distributed compilation fails, wrapper automatically falls back to local rustc
- Check worker logs for errors

### Build scripts fail
- Build scripts (build.rs) always run locally (intentional)
- Only library crates are distributed

## Performance

Expected speedup with 2 workers:
- **Sequential time**: ~5-10 seconds (all local)
- **Distributed time**: ~3-5 seconds (parallel stages)
- **Speedup**: ~1.5-2x (limited by dependency graph)

More workers = more parallelism for independent crates!

## Next Steps

Once this works, you can:
1. Add more workers (ports 6003, 6004, ...)
2. Test on a real multi-machine cluster
3. Replace dummy transformation with actual rustc execution
4. Add caching for repeated builds

---

**Ready? Start all 4 terminals and watch the magic happen!** 🚀

