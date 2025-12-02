use std::path::PathBuf;
use anyhow::Result;

/// Parsed rustc arguments
#[derive(Debug, Clone)]
pub struct RustcArgs {
    pub crate_name: Option<String>,
    pub is_lib: bool,
    pub input_files: Vec<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub original_args: Vec<String>,
}

impl RustcArgs {
    /// Parse rustc command-line arguments
    pub fn parse(args: &[String]) -> Result<Self> {
        let mut crate_name = None;
        let mut is_lib = false;
        let mut input_files = Vec::new();
        let mut output_path = None;
        
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            
            match arg.as_str() {
                "--crate-name" => {
                    if i + 1 < args.len() {
                        crate_name = Some(args[i + 1].clone());
                        i += 1;
                    }
                }
                "--crate-type" => {
                    if i + 1 < args.len() {
                        is_lib = args[i + 1] == "lib" || args[i + 1] == "rlib";
                        i += 1;
                    }
                }
                "-o" | "--out-dir" => {
                    if i + 1 < args.len() {
                        output_path = Some(PathBuf::from(&args[i + 1]));
                        i += 1;
                    }
                }
                _ => {
                    // Check if it's a .rs file (input)
                    if arg.ends_with(".rs") {
                        input_files.push(PathBuf::from(arg));
                    }
                }
            }
            
            i += 1;
        }
        
        Ok(RustcArgs {
            crate_name,
            is_lib,
            input_files,
            output_path,
            original_args: args.to_vec(),
        })
    }
}

