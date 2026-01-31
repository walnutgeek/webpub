use fastcdc::v2020::FastCDC;

/// A content-addressed chunk of data.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub hash: [u8; 32],
    pub data: Vec<u8>,
}

/// Chunk sizes: min 16KB, avg 32KB, max 64KB
const MIN_SIZE: u32 = 16 * 1024;
const AVG_SIZE: u32 = 32 * 1024;
const MAX_SIZE: u32 = 64 * 1024;

/// Chunk data using FastCDC algorithm, yielding chunks with BLAKE3 hashes.
pub fn chunk_data(data: &[u8]) -> impl Iterator<Item = Chunk> + '_ {
    let chunker = FastCDC::new(data, MIN_SIZE, AVG_SIZE, MAX_SIZE);

    chunker.map(|chunk| {
        let chunk_data = data[chunk.offset..chunk.offset + chunk.length].to_vec();
        let hash = *blake3::hash(&chunk_data).as_bytes();
        Chunk {
            hash,
            data: chunk_data,
        }
    })
}
