
use serde::Serialize;
use std::path::{Path, PathBuf};

pub struct FileOps {
    allowed_dirs: Vec<String>,
    max_file_size: u64,
}

#[derive(Debug, Serialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: i64,
    pub mode: u32,
}

impl FileOps {
    pub fn new(allowed_dirs: Vec<String>, max_file_size: u64) -> Self {
        Self { allowed_dirs, max_file_size }
    }

    fn validate_path(&self, path: &str) -> Result<PathBuf, &'static str> {
        if path.is_empty() {
            return Err(flowlink_core::codes::codes::FILE_EMPTY_PATH);
        }
        let p = Path::new(path);
        if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(flowlink_core::codes::codes::FILE_INVALID_PATH);
        }
        let canonical = match p.canonicalize() {
            Ok(c) => c,
            Err(_) => {
                if let Some(parent) = p.parent() {
                    if parent.as_os_str().is_empty() {
                        PathBuf::from(path)
                    } else {
                        parent.canonicalize().map(|c| c.join(p.file_name().unwrap_or_default())).unwrap_or_else(|_| PathBuf::from(path))
                    }
                } else {
                    return Err(flowlink_core::codes::codes::FILE_INVALID_PATH);
                }
            }
        };
        if !self.allowed_dirs.is_empty() {
            let allowed = self.allowed_dirs.iter().any(|dir| canonical.starts_with(dir));
            if !allowed {
                return Err(flowlink_core::codes::codes::FILE_INVALID_PATH);
            }
        }
        Ok(canonical)
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        let resolved = self.validate_path(path).map_err(|e| e.to_string())?;
        let metadata = std::fs::metadata(&resolved).map_err(|_| flowlink_core::codes::codes::FILE_NOT_FOUND.to_string())?;
        if metadata.len() > self.max_file_size {
            return Err(format!("{}: {} bytes > {} limit", flowlink_core::codes::codes::FILE_TOO_LARGE, metadata.len(), self.max_file_size));
        }
        if metadata.is_dir() {
            return Err(flowlink_core::codes::codes::FILE_READ_ERROR.to_string());
        }
        std::fs::read(&resolved).map_err(|e| format!("{}: {}", flowlink_core::codes::codes::FILE_READ_ERROR, e))
    }

    pub fn write(&self, path: &str, data: &[u8]) -> Result<(), String> {
        if data.len() as u64 > self.max_file_size {
            return Err(format!("{}: {} bytes > {} limit", flowlink_core::codes::codes::FILE_TOO_LARGE, data.len(), self.max_file_size));
        }
        let resolved = self.validate_path(path).map_err(|e| e.to_string())?;
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {}", flowlink_core::codes::codes::FILE_WRITE_ERROR, e))?;
        }
        std::fs::write(&resolved, data).map_err(|e| format!("{}: {}", flowlink_core::codes::codes::FILE_WRITE_ERROR, e))
    }

    pub fn list(&self, dir: &str, recursive: bool) -> Result<Vec<DirEntry>, String> {
        let resolved = self.validate_path(dir).map_err(|e| e.to_string())?;
        if !resolved.is_dir() {
            return Err(flowlink_core::codes::codes::FILE_NOT_FOUND.to_string());
        }
        let mut entries = Vec::new();
        self.list_dir(&resolved, &resolved, recursive, &mut entries)?;
        Ok(entries)
    }

    fn list_dir(&self, base: &Path, current: &Path, recursive: bool, out: &mut Vec<DirEntry>) -> Result<(), String> {
        let read_dir = std::fs::read_dir(current).map_err(|e| format!("{}: {}", flowlink_core::codes::codes::FILE_READ_ERROR, e))?;
        for entry in read_dir.flatten() {
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let rel = path.strip_prefix(base).unwrap_or(&path).to_string_lossy().to_string();
            let mode = mode_from_metadata(&metadata);
            out.push(DirEntry { name, path: rel, is_dir: metadata.is_dir(), size: metadata.len() as i64, mode });
            if recursive && metadata.is_dir() {
                self.list_dir(base, &path, true, out)?;
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
fn mode_from_metadata(m: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    m.mode()
}

#[cfg(not(unix))]
fn mode_from_metadata(_m: &std::fs::Metadata) -> u32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unrestricted_ops(dir: &str) -> FileOps {
        let canonical = std::fs::canonicalize(dir).unwrap();
        FileOps::new(vec![canonical.to_str().unwrap().into()], 1024 * 1024)
    }

    #[test]
    fn test_write_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let ops = unrestricted_ops(dir.path().to_str().unwrap());
        let full = dir.path().join("test.txt");
        ops.write(full.to_str().unwrap(), b"hello world").unwrap();
        let data = ops.read(full.to_str().unwrap()).unwrap();
        assert_eq!(data, b"hello world");
    }

    #[test]
    fn test_list_directory() {
        let dir = tempfile::tempdir().unwrap();
        let dir_str = dir.path().canonicalize().unwrap().to_str().unwrap().to_string();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::write(dir.path().join("b.txt"), "bb").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let ops = FileOps::new(vec![dir_str.clone()], 1024 * 1024);
        let entries = ops.list(&dir_str, false).unwrap();
        assert_eq!(entries.len(), 3);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.txt"));
        assert!(names.contains(&"sub"));
    }

    #[test]
    fn test_path_traversal_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let ops = unrestricted_ops(dir.path().to_str().unwrap());
        let result = ops.read("../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_size_limit_write() {
        let dir = tempfile::tempdir().unwrap();
        let ops = FileOps::new(vec![dir.path().to_str().unwrap().into()], 10);
        let result = ops.write("big.txt", b"this is way more than 10 bytes");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("FILE_TOO_LARGE"));
    }

    #[test]
    fn test_size_limit_read() {
        let dir = tempfile::tempdir().unwrap();
        let big_file = dir.path().join("big.txt");
        std::fs::write(&big_file, vec![0u8; 100]).unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        let ops = FileOps::new(vec![canonical.to_str().unwrap().into()], 50);
        let result = ops.read(big_file.to_str().unwrap());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("FILE_TOO_LARGE") || err.contains("too large"), "unexpected error: {err}");
    }

    #[test]
    fn test_allowed_dirs_restriction() {
        let dir = tempfile::tempdir().unwrap();
        let ops = FileOps::new(vec![dir.path().to_str().unwrap().into()], 1024);
        // Path outside allowed dirs
        let result = ops.read("/etc/hosts");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_path_rejected() {
        let ops = FileOps::new(vec![], 1024);
        assert!(ops.read("").is_err());
        assert!(ops.write("", b"x").is_err());
    }

    #[test]
    fn test_read_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let ops = unrestricted_ops(dir.path().to_str().unwrap());
        assert!(ops.read(dir.path().join("nope.txt").to_str().unwrap()).is_err());
    }
}
