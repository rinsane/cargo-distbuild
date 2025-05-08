use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

#[test]
fn integration_test_distbuild() {
    let root = Path::new("test_workspace");
    let crates_dir = root.join("crates");

    // Clean previous run
    let _ = fs::remove_dir_all(&root);

    // Create directory structure
    fs::create_dir_all(crates_dir.join("libA/src")).unwrap();
    fs::create_dir_all(crates_dir.join("libB/src")).unwrap();
    fs::create_dir_all(crates_dir.join("libC/src")).unwrap();
    fs::create_dir_all(crates_dir.join("bin/src")).unwrap();

    // Workspace Cargo.toml
    fs::write(
        root.join("Cargo.toml"),
        r#"[workspace]
members = [
    "crates/libA",
    "crates/libB",
    "crates/libC",
    "crates/bin"
]
"#,
    )
    .unwrap();

    // libA
    fs::write(
        crates_dir.join("libA/Cargo.toml"),
        r#"[package]
name = "libA"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(
        crates_dir.join("libA/src/lib.rs"),
        r#"pub fn say_hello() {
    println!("Hello from libA!");
}
"#,
    )
    .unwrap();

    // libC
    fs::write(
        crates_dir.join("libC/Cargo.toml"),
        r#"[package]
name = "libC"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(
        crates_dir.join("libC/src/lib.rs"),
        r#"pub fn say_hi() {
    println!("Hi from libC!");
}
"#,
    )
    .unwrap();

    // libB (depends on libC)
    fs::write(
        crates_dir.join("libB/Cargo.toml"),
        r#"[package]
name = "libB"
version = "0.1.0"
edition = "2021"

[dependencies]
libC = { path = "../libC" }
"#,
    )
    .unwrap();
    fs::write(
        crates_dir.join("libB/src/lib.rs"),
        r#"use libC::say_hi;

pub fn say_something() {
    say_hi();
    println!("...and hello from libB!");
}
"#,
    )
    .unwrap();

    // Binary crate (depends on libA and libB)
    fs::write(
        crates_dir.join("bin/Cargo.toml"),
        r#"[package]
name = "bin"
version = "0.1.0"
edition = "2021"

[dependencies]
libA = { path = "../libA" }
libB = { path = "../libB" }
"#,
    )
    .unwrap();
    fs::write(
        crates_dir.join("bin/src/main.rs"),
        r#"use libA::say_hello;
use libB::say_something;

fn main() {
    say_hello();
    say_something();
}
"#,
    )
    .unwrap();

    // ✅ 1. Run `cargo-distbuild` on this project
    let distbuild_status = Command::new("cargo")
        .arg("run")
        .args(&["--bin", "cargo-distbuild"])
        .arg("--")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .status()
        .expect("failed to run cargo-distbuild");

    assert!(distbuild_status.success(), "cargo-distbuild failed");

    // ✅ 2. Try to build and run the final binary
    let build_status = Command::new("cargo")
        .current_dir(root)
        .arg("run")
        .arg("-p")
        .arg("bin")
        .status()
        .expect("failed to run final binary crate");

    assert!(build_status.success(), "final binary crate failed to build or run");
}
