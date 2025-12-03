# cargo-distbuild

**A distributed compilation system for Rust projects**

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()

---

## 🚀 Overview

`cargo-distbuild` is a distributed build system designed to speed up Rust compilation by distributing work across multiple machines. It uses a **Content-Addressable Storage (CAS)** backend for efficient artifact sharing and **gRPC** for coordination.

### Key Features

- ✅ **Content-Addressable Storage**: Deduplicated, hash-based artifact storage
- ✅ **Distributed Execution**: Submit jobs to a pool of workers
- ✅ **gRPC Communication**: Efficient, typed RPC for control plane
- ✅ **Interactive CLI**: Both command-line and REPL interfaces
- ✅ **Worker Pool Management**: Automatic load balancing
- 🚧 **Cargo Integration**: Coming soon via `RUSTC_WORKSPACE_WRAPPER`
- 🚧 **Docker Isolation**: Hermetic builds in containers

## 📚 Documentation

- **[QUICKSTART.md](markdowns/iteration_1/QUICKSTART.md)** - Get up and running in 5 minutes
- **[NEW_ARCHITECTURE.md](markdowns/iteration_1/NEW_ARCHITECTURE.md)** - Complete architectural overview
- **[docs/plan/](docs/plan/)** - Design documents and planning
- **[markdowns/](markdowns/)** - Documentation by iteration

## 🏗️ Current Status: Phase 2 (Skeleton System)

This is a **complete rewrite** of the original Phase-1 prototype. The current implementation provides:

✅ **Complete**:
- Filesystem-based CAS implementation
- Scheduler service with worker management
- Worker service with job execution
- Master CLI with interactive REPL
- gRPC-based communication
- Integration tests

⏳ **In Progress**:
- Cargo wrapper integration
- Real Rust compilation
- Docker container execution
- Action caching

## 🎯 Quick Start

### 1. Build the Project

```bash
cargo build --release
```

### 2. Run the System

**Terminal 1 - Start Scheduler:**
```bash
cargo run -- scheduler run
```

**Terminal 2 - Start Worker:**
```bash
cargo run -- worker run --id worker-1 --port 6001
```

**Terminal 3 - Interactive Shell:**
```bash
cargo run
```

### 3. Try It Out

In the interactive shell:

```
cargo-distbuild> help
cargo-distbuild> cas put README.md
cargo-distbuild> job submit <hash-from-above>
cargo-distbuild> jobs list
cargo-distbuild> workers list
```

## 🏛️ Architecture

```
┌─────────────┐      gRPC       ┌─────────────┐
│   Master    │◄────────────────►│  Scheduler  │
│   (CLI)     │                  │             │
└─────────────┘                  └─────────────┘
       │                                │
       │                                │ gRPC
       │                                ▼
       │                         ┌─────────────┐
       │                         │  Worker 1   │
       │                         │  Worker 2   │
       │                         │  Worker N   │
       │                         └─────────────┘
       │                                │
       │         Filesystem             │
       └────────────►CAS◄───────────────┘
```

### Components

- **Master**: Developer-facing CLI (interactive and batch modes)
- **Scheduler**: Central coordinator for job distribution
- **Workers**: Execute compilation jobs on remote machines
- **CAS**: Content-addressable storage for all artifacts

## 🛠️ Usage

### Command-Line Interface

```bash
# CAS operations
cargo-distbuild cas put <file>
cargo-distbuild cas get <hash> <output>
cargo-distbuild cas list

# Run services
cargo-distbuild scheduler run
cargo-distbuild worker run --id worker-1 --port 6001

# Job management
cargo-distbuild master submit-job <input-hash>
cargo-distbuild master job-status <job-id>
cargo-distbuild master list-jobs
cargo-distbuild master list-workers
```

### Interactive REPL

Start with no arguments:
```bash
cargo run
```

Available commands:
- `cas put <file>` - Store a file in CAS
- `cas get <hash> <out>` - Retrieve from CAS
- `cas list` - List all hashes
- `job submit <hash>` - Submit a job
- `job status <id>` - Check job status
- `jobs list` - List recent jobs
- `workers list` - Show registered workers
- `scheduler status` - Scheduler info
- `help` - Show all commands
- `exit` - Quit

## ⚙️ Configuration

Edit `config.toml`:

```toml
[scheduler]
addr = "127.0.0.1:5000"

[cas]
root = "./cas-root"

[worker]
heartbeat_interval_secs = 10
capacity = 4
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run unit tests only
cargo test --lib

# Run integration tests
cargo test --test integration

# Run a specific test
cargo test test_end_to_end_workflow
```

## 📖 How It Works

### Current Workflow (Dummy Jobs)

1. **Store Input**: Put data in CAS → get hash
2. **Submit Job**: Master sends job to Scheduler with input hash
3. **Assign Work**: Scheduler assigns job to available Worker
4. **Execute**: Worker reads from CAS, processes, writes back to CAS
5. **Retrieve**: Master gets output from CAS using output hash

### Future Workflow (With Cargo)

1. **Developer runs**: `cargo build`
2. **Cargo calls**: Our wrapper instead of `rustc`
3. **Wrapper**:
   - Hashes input files
   - Uploads to CAS
   - Submits compilation job to Scheduler
4. **Worker**:
   - Downloads inputs from CAS
   - Runs `rustc` in Docker
   - Uploads outputs to CAS
5. **Wrapper**:
   - Downloads outputs
   - Writes to `target/` where Cargo expects
6. **Cargo**: Continues with next crate

## 🔄 Roadmap

### Phase 2: Skeleton System ✅ **CURRENT**
- [x] CAS implementation
- [x] gRPC services
- [x] Worker management
- [x] Job distribution
- [x] CLI and REPL

### Phase 3: Cargo Integration 🚧
- [x] `RUSTC_WORKSPACE_WRAPPER` implementation
- [x] Parse rustc arguments
- [x] Action fingerprinting
- [x] Integration with Cargo build


## 🤝 Contributing

This is currently a research/academic project (BTP - Bachelor Thesis Project). 

See `docs/` for design documents and architecture details.

## 📄 License

MIT License - See LICENSE file for details

## 🔗 Related Projects

- **[sccache](https://github.com/mozilla/sccache)** - Shared compilation cache
- **[distcc](https://github.com/distcc/distcc)** - Distributed C/C++ compilation
- **[Bazel](https://bazel.build)** - Build system with remote execution

## 📧 Contact

For questions or feedback, see the project documentation in `docs/`.

---

**Note**: This is Phase 2 of the project. Phase 1 (HTTP/tarball-based) has been completely replaced with this new CAS/gRPC architecture. See `NEW_ARCHITECTURE.md` for details on the redesign.
