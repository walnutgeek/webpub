use std::fs;
use std::io;
use std::path::Path;

/// A scanned filesystem entry.
#[derive(Debug)]
pub enum ScannedEntry {
    File {
        name: String,
        permissions: u32,
        size: u64,
        data: Vec<u8>,
    },
    Directory {
        name: String,
        permissions: u32,
        children: Vec<ScannedEntry>,
    },
}

impl ScannedEntry {
    pub fn name(&self) -> &str {
        match self {
            ScannedEntry::File { name, .. } => name,
            ScannedEntry::Directory { name, .. } => name,
        }
    }
}

/// Scan a directory recursively, returning entries sorted by name.
/// Ignores symlinks and special files.
/// Returns an iterator yielding the root entry as a tree structure.
pub fn scan_directory(path: &Path) -> io::Result<impl Iterator<Item = ScannedEntry>> {
    let entry = scan_entry(path, "")?;
    Ok(std::iter::once(entry))
}

fn scan_entry(path: &Path, name: &str) -> io::Result<ScannedEntry> {
    let metadata = fs::metadata(path)?;

    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode()
    };
    #[cfg(not(unix))]
    let permissions = if metadata.permissions().readonly() { 0o444 } else { 0o644 };

    if metadata.is_file() {
        let data = fs::read(path)?;
        Ok(ScannedEntry::File {
            name: name.to_string(),
            permissions,
            size: metadata.len(),
            data,
        })
    } else if metadata.is_dir() {
        let mut children = Vec::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_type = entry.file_type()?;

            // Skip symlinks and special files
            if file_type.is_symlink() {
                continue;
            }

            let child_name = entry.file_name().to_string_lossy().to_string();
            let child_path = entry.path();

            // Skip if we can't read metadata (broken symlink, permission denied, etc.)
            if let Ok(child_entry) = scan_entry(&child_path, &child_name) {
                children.push(child_entry);
            }
        }

        // Sort by name for determinism
        children.sort_by(|a, b| a.name().cmp(b.name()));

        Ok(ScannedEntry::Directory {
            name: name.to_string(),
            permissions,
            children,
        })
    } else {
        // Special file - treat as empty directory to skip
        Err(io::Error::new(io::ErrorKind::Other, "special file"))
    }
}
