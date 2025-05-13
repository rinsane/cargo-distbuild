#![allow(unused_imports)]
use std::{
    fs::{self, File},
    io::{Write, BufWriter},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
    os::unix::fs::PermissionsExt,
};
use anyhow::Result;

#[test]
fn integration_test_distbuild() -> Result<()> {
    // 1. Create test workspace in target directory
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test_workspace");
    let crates_dir = root.join("crates");
    println!("Working Directory: {:?}", root);

    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(crates_dir.join("libA/src"))?;
    fs::create_dir_all(crates_dir.join("libB/src"))?;
    fs::create_dir_all(crates_dir.join("libC/src"))?;
    fs::create_dir_all(crates_dir.join("bin/src"))?;

    // nodes.json
    let mut f = File::create(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("nodes.json"))?;
    f.write_all(br#"[{"ip": "127.0.0.1", "port": 5001}, {"ip": "127.0.0.1", "port": 5002}]"#)?;
    f.flush()?;

    // Workspace Cargo.toml
    let mut f = File::create(root.join("Cargo.toml"))?;
    f.write_all(
        br#"[workspace]
resolver = "2"
members = [
    "crates/libA",
    "crates/libB",
    "crates/libC",
    "crates/bin"
]
"#,
    )?;
    f.flush()?;

    // libA
    let mut f = File::create(crates_dir.join("libA/Cargo.toml"))?;
    f.write_all(
        br#"[package]
name = "libA"
version = "0.1.0"
edition = "2021"
"#,
    )?;
    f.flush()?;

    let mut f = File::create(crates_dir.join("libA/src/lib.rs"))?;
    f.write_all(
        br#"pub fn say_hello() {
    println!("Hello from libA!");
}
"#,
    )?;
    f.flush()?;

    // libC
    let mut f = File::create(crates_dir.join("libC/Cargo.toml"))?;
    f.write_all(
        br#"[package]
name = "libC"
version = "0.1.0"
edition = "2021"
"#,
    )?;
    f.flush()?;

    let mut f = File::create(crates_dir.join("libC/src/lib.rs"))?;
    f.write_all(
        br#"pub fn say_hi() {
    println!("Hi from libC!");
}
"#,
    )?;
    f.flush()?;

    // libB (depends on libC)
    let mut f = File::create(crates_dir.join("libB/Cargo.toml"))?;
    f.write_all(
        br#"[package]
name = "libB"
version = "0.1.0"
edition = "2021"

[dependencies]
libC = { path = "../libC" }
"#,
    )?;
    f.flush()?;

    let mut f = File::create(crates_dir.join("libB/src/lib.rs"))?;
    f.write_all(
        br#"use libC::say_hi;

pub fn say_something() {
    say_hi();
    println!("...and hello from libB!");
}
"#,
    )?;
    f.flush()?;

    // bin
    let mut f = File::create(crates_dir.join("bin/Cargo.toml"))?;
    f.write_all(
        br#"[package]
name = "bin"
version = "0.1.0"
edition = "2021"

[dependencies]
libA = { path = "../libA" }
libB = { path = "../libB" }
"#,
    )?;
    f.flush()?;

    let mut f = File::create(crates_dir.join("bin/src/main.rs"))?;
    f.write_all(
        br#"use libA::say_hello;
use libB::say_something;

fn main() {
    say_hello();
    say_something();
}
"#,
    )?;
    f.flush()?;

    // Optional: short sleep to allow disk sync (good for safety on CI)
    thread::sleep(Duration::from_millis(300));

    // Run cargo-distbuild
    let distbuild_bin = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("cargo-distbuild");
    println!("Cargo-distbuild run at: {:?}", distbuild_bin);

    let distbuild_status = Command::new(distbuild_bin)
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .status()
        .expect("failed to run cargo-distbuild");

    assert!(distbuild_status.success(), "cargo-distbuild failed");

    // // Build final binary crate using offline Cargo
    // let build_status = Command::new("cargo")
    //     .current_dir(&root)
    //     .arg("build")
    //     .arg("-p")
    //     .arg("bin")
    //     .arg("--offline")
    //     .status()
    //     .expect("failed to build binary crate");
    // assert!(build_status.success(), "final binary crate failed to build");

    // Run final binary crate
    let bin_path = root.join("target/debug/bin");
    
    // Set executable permissions
    let mut perms = fs::metadata(&bin_path)?.permissions();
    perms.set_mode(0o755); // rwxr-xr-x
    fs::set_permissions(&bin_path, perms)?;

    let output = Command::new(&bin_path)
        .current_dir(&root)
        .output()
        .expect("failed to run binary crate");

    assert!(
        output.status.success(),
        "running bin crate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify stdout
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello from libA!"), "Missing libA output");
    assert!(stdout.contains("Hi from libC!"), "Missing libC output");
    assert!(stdout.contains("...and hello from libB!"), "Missing libB output");

    println!("\n🎉 Binary output:\n{}", stdout);
    Ok(())
}
