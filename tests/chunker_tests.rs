use webpub::chunker::{chunk_data, Chunk};

#[test]
fn test_chunk_small_data() {
    let data = b"Hello, world!";
    let chunks: Vec<Chunk> = chunk_data(data).collect();

    // Small data should produce one chunk
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].data, data);

    // Hash should be BLAKE3 of data
    let expected_hash = blake3::hash(data);
    assert_eq!(chunks[0].hash, *expected_hash.as_bytes());
}

#[test]
fn test_chunk_deterministic() {
    let data = b"Some test data that we chunk";
    let chunks1: Vec<Chunk> = chunk_data(data).collect();
    let chunks2: Vec<Chunk> = chunk_data(data).collect();

    assert_eq!(chunks1.len(), chunks2.len());
    for (c1, c2) in chunks1.iter().zip(chunks2.iter()) {
        assert_eq!(c1.hash, c2.hash);
    }
}

#[test]
fn test_chunk_large_data() {
    // Create data large enough to produce multiple chunks
    // fastcdc default min is 16KB, avg 32KB, max 64KB
    let data: Vec<u8> = (0..200_000).map(|i| (i % 256) as u8).collect();
    let chunks: Vec<Chunk> = chunk_data(&data).collect();

    // Should produce multiple chunks
    assert!(chunks.len() > 1);

    // Reconstruct and verify
    let reconstructed: Vec<u8> = chunks.iter().flat_map(|c| c.data.iter().copied()).collect();
    assert_eq!(reconstructed, data);
}
