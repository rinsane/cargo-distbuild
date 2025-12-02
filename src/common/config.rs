use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub scheduler: SchedulerConfig,
    pub cas: CasConfig,
    pub worker: WorkerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CasConfig {
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub heartbeat_interval_secs: u64,
    pub capacity: u32,
}

impl Config {
    /// Load config from a TOML file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file {:?}", path.as_ref()))?;
        
        let config: Config = toml::from_str(&content)
            .with_context(|| "Failed to parse config file")?;
        
        Ok(config)
    }

    /// Load config from default locations
    pub fn load_default() -> Result<Self> {
        // Try current directory first
        if Path::new("config.toml").exists() {
            return Self::load("config.toml");
        }

        // Try ~/.config/cargo-distbuild/config.toml
        if let Some(home) = std::env::var_os("HOME") {
            let config_path = Path::new(&home)
                .join(".config")
                .join("cargo-distbuild")
                .join("config.toml");
            if config_path.exists() {
                return Self::load(config_path);
            }
        }

        // Return default config
        Ok(Self::default())
    }

    /// Save config to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize config")?;
        
        fs::write(path.as_ref(), content)
            .with_context(|| format!("Failed to write config to {:?}", path.as_ref()))?;
        
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            scheduler: SchedulerConfig {
                addr: "127.0.0.1:5000".to_string(),
            },
            cas: CasConfig {
                root: "./cas-root".to_string(),
            },
            worker: WorkerConfig {
                heartbeat_interval_secs: 10,
                capacity: 4,
            },
        }
    }
}

