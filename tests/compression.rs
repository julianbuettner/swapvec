use swapvec::{Compression, CompressionLevel, SwapVec, SwapVecConfig};

#[test]
fn write_and_read_back_with_compression() {
    let data: Vec<i32> = (0..999).collect();

    let compression_configs: Vec<Option<Compression>> = vec![
        None,
        Some(Compression::Lz4),
        Some(Compression::Deflate(CompressionLevel::Fast)),
        Some(Compression::Deflate(CompressionLevel::Default)),
        Some(Compression::Deflate(CompressionLevel::Slow)),
    ];

    for compression in compression_configs {
        let config = SwapVecConfig {
            compression,
            swap_after: 16,
            batch_size: 8,
        };
        let mut v = SwapVec::with_config(config);
        v.consume(data.iter().map(|x| *x)).unwrap();
        let read_back: Vec<i32> = v
            .into_iter()
            .map(|x| {
                x.unwrap_or_else(|e| panic!("Failed for compression {:?} {:?}", compression, e))
            })
            .collect();
        assert_eq!(read_back, data,);
    }
}
