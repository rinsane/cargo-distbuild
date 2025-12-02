use crate::common::Config;
use crate::master::commands::CommandExecutor;
use anyhow::Result;
use colored::*;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result as RustyResult};

pub async fn run_repl() -> Result<()> {
    println!("{}", "🚀 cargo-distbuild interactive shell".bright_green().bold());
    println!("Type 'help' for available commands, 'exit' to quit\n");

    let config = Config::load_default()?;
    let executor = CommandExecutor::new(config)?;

    let mut rl: DefaultEditor = DefaultEditor::new()?;
    
    // Load history if available
    let history_file = dirs::home_dir()
        .map(|h| h.join(".cargo-distbuild-history"));
    
    if let Some(ref path) = history_file {
        let _ = rl.load_history(path);
    }

    loop {
        let readline = rl.readline("cargo-distbuild> ");
        
        match readline {
            Ok(line) => {
                let line = line.trim();
                
                if line.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(line);

                if let Err(e) = handle_command(&executor, line).await {
                    eprintln!("{} {}", "Error:".red().bold(), e);
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("exit");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    if let Some(ref path) = history_file {
        let _ = rl.save_history(path);
    }

    println!("{}", "Goodbye! 👋".bright_green());
    Ok(())
}

async fn handle_command(executor: &CommandExecutor, line: &str) -> Result<()> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    
    if parts.is_empty() {
        return Ok(());
    }

    match parts[0] {
        "help" => {
            executor.show_help();
        }
        "exit" | "quit" => {
            println!("{}", "Goodbye! 👋".bright_green());
            std::process::exit(0);
        }
        "cas" => {
            if parts.len() < 2 {
                eprintln!("Usage: cas <put|get|exists|list> [args...]");
                return Ok(());
            }
            
            match parts[1] {
                "put" => {
                    if parts.len() < 3 {
                        eprintln!("Usage: cas put <file>");
                        return Ok(());
                    }
                    executor.cas_put(parts[2]).await?;
                }
                "get" => {
                    if parts.len() < 4 {
                        eprintln!("Usage: cas get <hash> <output-file>");
                        return Ok(());
                    }
                    executor.cas_get(parts[2], parts[3]).await?;
                }
                "exists" => {
                    if parts.len() < 3 {
                        eprintln!("Usage: cas exists <hash>");
                        return Ok(());
                    }
                    executor.cas_exists(parts[2]).await?;
                }
                "list" => {
                    executor.cas_list().await?;
                }
                _ => {
                    eprintln!("Unknown cas subcommand: {}", parts[1]);
                    eprintln!("Available: put, get, exists, list");
                }
            }
        }
        "job" => {
            if parts.len() < 2 {
                eprintln!("Usage: job <submit|status> [args...]");
                return Ok(());
            }
            
            match parts[1] {
                "submit" => {
                    if parts.len() < 3 {
                        eprintln!("Usage: job submit <input-hash>");
                        return Ok(());
                    }
                    executor.submit_job(parts[2]).await?;
                }
                "status" => {
                    if parts.len() < 3 {
                        eprintln!("Usage: job status <job-id>");
                        return Ok(());
                    }
                    executor.job_status(parts[2]).await?;
                }
                _ => {
                    eprintln!("Unknown job subcommand: {}", parts[1]);
                    eprintln!("Available: submit, status");
                }
            }
        }
        "jobs" => {
            if parts.len() < 2 {
                eprintln!("Usage: jobs list [limit]");
                return Ok(());
            }
            
            match parts[1] {
                "list" => {
                    let limit = if parts.len() >= 3 {
                        parts[2].parse().unwrap_or(10)
                    } else {
                        10
                    };
                    executor.list_jobs(limit).await?;
                }
                _ => {
                    eprintln!("Unknown jobs subcommand: {}", parts[1]);
                    eprintln!("Available: list");
                }
            }
        }
        "workers" => {
            if parts.len() < 2 {
                eprintln!("Usage: workers list");
                return Ok(());
            }
            
            match parts[1] {
                "list" => {
                    executor.list_workers().await?;
                }
                _ => {
                    eprintln!("Unknown workers subcommand: {}", parts[1]);
                    eprintln!("Available: list");
                }
            }
        }
        "scheduler" => {
            if parts.len() < 2 {
                eprintln!("Usage: scheduler status");
                return Ok(());
            }
            
            match parts[1] {
                "status" => {
                    executor.scheduler_status().await?;
                }
                _ => {
                    eprintln!("Unknown scheduler subcommand: {}", parts[1]);
                    eprintln!("Available: status");
                }
            }
        }
        _ => {
            eprintln!("Unknown command: {}", parts[0]);
            eprintln!("Type 'help' for available commands");
        }
    }

    Ok(())
}

