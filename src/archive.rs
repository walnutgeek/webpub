use crate::chunker::Chunk;
use crate::merkle::Node;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
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
    let index_bytes = rmp_serde::to_vec(&index)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
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
