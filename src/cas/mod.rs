use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Content-Addressable Storage (CAS)
/// Layout: <cas_root>/<first2>/<next2>/<full_sha256>
#[derive(Debug, Clone)]
pub struct Cas {
    root: PathBuf,
}

impl Cas {
    /// Create a new CAS instance
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)
            .with_context(|| format!("Failed to create CAS root at {:?}", root))?;
        Ok(Cas { root })
    }

    /// Put bytes into CAS and return the hash
    pub fn put(&self, data: &[u8]) -> Result<String> {
        let hash = self.compute_hash(data);
        let path = self.hash_to_path(&hash);
        
        // Create parent directories
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }

        // Write the blob (skip if already exists)
        if !path.exists() {
            let mut file = fs::File::create(&path)
                .with_context(|| format!("Failed to create file {:?}", path))?;
            file.write_all(data)
                .with_context(|| format!("Failed to write to {:?}", path))?;
        }

        Ok(hash)
    }

    /// Get bytes from CAS by hash
    pub fn get(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.hash_to_path(hash);
        
        if !path.exists() {
            anyhow::bail!("Hash {} not found in CAS", hash);
        }

        let mut file = fs::File::open(&path)
            .with_context(|| format!("Failed to open {:?}", path))?;
        
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .with_context(|| format!("Failed to read from {:?}", path))?;

        Ok(data)
    }

    /// Check if a hash exists in CAS
    pub fn exists(&self, hash: &str) -> bool {
        self.hash_to_path(hash).exists()
    }

    /// Get the file path for a hash (without checking existence)
    pub fn get_path(&self, hash: &str) -> PathBuf {
        self.hash_to_path(hash)
    }

    /// Compute SHA-256 hash of data
    fn compute_hash(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Convert hash to filesystem path
    /// Layout: <root>/<first2>/<next2>/<full_hash>
    fn hash_to_path(&self, hash: &str) -> PathBuf {
        if hash.len() < 4 {
            return self.root.join(hash);
        }
        
        let first2 = &hash[0..2];
        let next2 = &hash[2..4];
        
        self.root.join(first2).join(next2).join(hash)
    }

    /// List all hashes in CAS (for debugging/testing)
    pub fn list_all(&self) -> Result<Vec<String>> {
        let mut hashes = Vec::new();
        
        if !self.root.exists() {
            return Ok(hashes);
        }

        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let first2_path = entry.path();
            
            if !first2_path.is_dir() {
                continue;
            }

            for entry in fs::read_dir(&first2_path)? {
                let entry = entry?;
                let next2_path = entry.path();
                
                if !next2_path.is_dir() {
                    continue;
                }

                for entry in fs::read_dir(&next2_path)? {
                    let entry = entry?;
                    if entry.path().is_file() {
                        if let Some(hash) = entry.file_name().to_str() {
                            hashes.push(hash.to_string());
                        }
                    }
                }
            }
        }

        Ok(hashes)
    }

    /// Get CAS root directory
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cas_put_get() {
        let temp_dir = TempDir::new().unwrap();
        let cas = Cas::new(temp_dir.path()).unwrap();

        let data = b"hello world";
        let hash = cas.put(data).unwrap();
        
        assert_eq!(hash.len(), 64); // SHA-256 is 64 hex chars
        assert!(cas.exists(&hash));

        let retrieved = cas.get(&hash).unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn test_cas_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let cas = Cas::new(temp_dir.path()).unwrap();

        let fake_hash = "0".repeat(64);
        assert!(!cas.exists(&fake_hash));
        assert!(cas.get(&fake_hash).is_err());
    }

    #[test]
    fn test_cas_deduplication() {
        let temp_dir = TempDir::new().unwrap();
        let cas = Cas::new(temp_dir.path()).unwrap();

        let data = b"duplicate content";
        let hash1 = cas.put(data).unwrap();
        let hash2 = cas.put(data).unwrap();
        
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_cas_list_all() {
        let temp_dir = TempDir::new().unwrap();
        let cas = Cas::new(temp_dir.path()).unwrap();

        let hash1 = cas.put(b"content1").unwrap();
        let hash2 = cas.put(b"content2").unwrap();
        
        let all_hashes = cas.list_all().unwrap();
        assert_eq!(all_hashes.len(), 2);
        assert!(all_hashes.contains(&hash1));
        assert!(all_hashes.contains(&hash2));
    }
}

