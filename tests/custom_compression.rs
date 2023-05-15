use swapvec::{Compress, CompressBoxedClone, Compression, SwapVec, SwapVecConfig};

#[derive(Debug)]
struct MyCompression;

impl Compress for MyCompression {
    fn compress(&self, block: Vec<u8>) -> Vec<u8> {
        block
    }
    fn decompress(&self, block: Vec<u8>) -> Result<Vec<u8>, ()> {
        Ok(block)
    }
}

impl CompressBoxedClone for MyCompression {
    fn boxed_clone(&self) -> Box<dyn CompressBoxedClone> {
        Box::new(MyCompression)
    }
}

#[test]
fn custom_compression() {
    let config = SwapVecConfig {
        compression: Some(Compression::Custom(Box::new(MyCompression))),
        swap_after: 16,
        batch_size: 5,
    };

    let vector: Vec<u64> = (0..999).collect();
    let mut v = SwapVec::with_config(config);
    v.consume(vector.clone().into_iter()).unwrap();
    assert!(v.written_to_file());
    let vector_read_back: Vec<u64> = v.into_iter().map(|x| x.unwrap()).collect();
    assert_eq!(vector, vector_read_back);
}
