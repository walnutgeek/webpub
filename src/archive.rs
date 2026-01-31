use crate::chunker::Chunk;
use crate::merkle::Node;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const MAGIC: &[u8; 8] = b"WEBPUB\0\0";
pub const VERSION: u8 = 1;

/// Header size: magic (8) + version (1) + index_offset (8) + index_size (8) = 25 bytes
const HEADER_SIZE: u64 = 25;

/// Archive index stored at the end of the file.
#[derive(Serialize, Deserialize)]
pub struct ArchiveIndex {
    pub tree: Node,
    pub chunk_offsets: HashMap<[u8; 32], (u64, u64)>, // hash -> (offset, size)
}

/// Write an archive file.
pub fn write_archive(path: &Path, tree: &Node, chunks: &[Chunk]) -> io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    // Write placeholder header
    writer.write_all(MAGIC)?;
    writer.write_all(&[VERSION])?;
    writer.write_all(&[0u8; 16])?; // placeholder for index_offset and index_size

    // Write chunks, tracking offsets (deduplicate by hash)
    let mut chunk_offsets: HashMap<[u8; 32], (u64, u64)> = HashMap::new();
    let mut offset = HEADER_SIZE;

    for chunk in chunks {
        if chunk_offsets.contains_key(&chunk.hash) {
            continue; // Skip duplicate
        }

        writer.write_all(&chunk.data)?;
        chunk_offsets.insert(chunk.hash, (offset, chunk.data.len() as u64));
        offset += chunk.data.len() as u64;
    }

    // Write index
    let index = ArchiveIndex {
        tree: tree.clone(),
        chunk_offsets,
    };
    let index_bytes =
        rmp_serde::to_vec(&index).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let index_offset = offset;
    let index_size = index_bytes.len() as u64;
    writer.write_all(&index_bytes)?;

    // Seek back and write actual header
    writer.flush()?;
    let mut file = writer.into_inner()?;
    file.seek(SeekFrom::Start(9))?; // After magic + version
    file.write_all(&index_offset.to_le_bytes())?;
    file.write_all(&index_size.to_le_bytes())?;

    Ok(())
}

/// Read and extract an archive file.
pub fn read_archive(archive_path: &Path, output_path: &Path) -> io::Result<()> {
    let file = File::open(archive_path)?;
    let mut reader = BufReader::new(file);

    // Read and verify header
    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
    }

    let mut version = [0u8; 1];
    reader.read_exact(&mut version)?;
    if version[0] != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported version",
        ));
    }

    let mut offset_bytes = [0u8; 8];
    reader.read_exact(&mut offset_bytes)?;
    let index_offset = u64::from_le_bytes(offset_bytes);

    let mut size_bytes = [0u8; 8];
    reader.read_exact(&mut size_bytes)?;
    let index_size = u64::from_le_bytes(size_bytes);

    // Read index
    reader.seek(SeekFrom::Start(index_offset))?;
    let mut index_bytes = vec![0u8; index_size as usize];
    reader.read_exact(&mut index_bytes)?;

    let index: ArchiveIndex = rmp_serde::from_slice(&index_bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Extract tree
    fs::create_dir_all(output_path)?;
    extract_node(&index.tree, output_path, &mut reader, &index.chunk_offsets)?;

    Ok(())
}

fn extract_node(
    node: &Node,
    base_path: &Path,
    reader: &mut BufReader<File>,
    chunk_offsets: &HashMap<[u8; 32], (u64, u64)>,
) -> io::Result<()> {
    match node {
        Node::File {
            name,
            chunks,
            permissions,
            ..
        } => {
            let file_path = base_path.join(name);
            let mut file = File::create(&file_path)?;

            for hash in chunks {
                let (offset, size) = chunk_offsets
                    .get(hash)
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing chunk"))?;

                reader.seek(SeekFrom::Start(*offset))?;
                let mut data = vec![0u8; *size as usize];
                reader.read_exact(&mut data)?;
                file.write_all(&data)?;
            }

            // Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&file_path, fs::Permissions::from_mode(*permissions))?;
            }
        }
        Node::Directory {
            name,
            children,
            permissions,
            ..
        } => {
            let dir_path = if name.is_empty() {
                base_path.to_path_buf()
            } else {
                base_path.join(name)
            };

            fs::create_dir_all(&dir_path)?;

            for child in children {
                extract_node(child, &dir_path, reader, chunk_offsets)?;
            }

            // Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&dir_path, fs::Permissions::from_mode(*permissions))?;
            }
        }
    }
    Ok(())
}
