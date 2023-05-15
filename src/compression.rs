use lz4_flex::{compress_prepend_size, decompress_size_prepended};

use crate::{swapvec::CompressionLevel, Compression};

/// Provide your own compression algorithm by
/// creating an empty struct implementing `compress`
/// and `decompress`.
///
/// Your compression algorithm is allowed to fail,
/// but _must_ always decompress into the same
/// bytes. Undefined behaviour otherwise.
///
/// Note: You must always also implement
/// CompressBoxedClone, to allow cloning
/// and debugging of the configuration.
///
/// ```rust
/// use swapvec::Compress;
/// struct DummyCompression;
/// impl Compress for DummyCompression {
///   fn compress(&self, block: Vec<u8>) -> Vec<u8> {
///       block
///   }
///   fn decompress(&self, block: Vec<u8>) -> Result<Vec<u8>, ()> {
///       Ok(block)
///   }
/// }
///
/// let bytes = vec![1, 2, 3];
/// let compression = DummyCompression;
/// assert_eq!(bytes, compression.decompress(compression.compress(bytes.clone())).unwrap());
/// ```
pub trait Compress {
    /// Compress bytes blockwise. The compressed block
    /// will be put into `self.decompress()` later.
    fn compress(&self, block: Vec<u8>) -> Vec<u8>;
    /// Receive block which was earlier `compress()`ed.
    /// If the result is `Ok`, the same bytes which were
    /// `compress()`es earlier are expected.
    fn decompress(&self, block: Vec<u8>) -> Result<Vec<u8>, ()>;
}

impl Compress for Option<Compression> {
    fn compress(&self, block: Vec<u8>) -> Vec<u8> {
        match self {
            Some(Compression::Lz4) => compress_prepend_size(&block).to_vec(),
            Some(Compression::Deflate(level)) => {
                let compression_level = match level {
                    CompressionLevel::Fast => 2,
                    CompressionLevel::Default => 6,
                    CompressionLevel::Slow => 9,
                };
                miniz_oxide::deflate::compress_to_vec(&block, compression_level)
            }
            Some(Compression::Custom(algo)) => algo.compress(block),
            None => block,
        }
    }
    fn decompress(&self, block: Vec<u8>) -> Result<Vec<u8>, ()> {
        match self {
            Some(Compression::Lz4) => decompress_size_prepended(&block).map_err(|_| ()),
            Some(Compression::Deflate(_)) => {
                miniz_oxide::inflate::decompress_to_vec(&block).map_err(|_| ())
            }
            Some(Compression::Custom(algo)) => algo.decompress(block),
            None => Ok(block),
        }
    }
}

/// Your custom compression algorithm struct must be debugable
/// and clonable. Implement this trait to keep the main
/// configuration debugable and clonable.
pub trait CompressBoxedClone: Compress + std::fmt::Debug {
    /// Clone your empty struct and return it as a new Box.
    fn boxed_clone(&self) -> Box<dyn CompressBoxedClone>;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_lz4() {
        let compression = Some(Compression::Lz4);
        let data: Vec<u8> = (0..u8::MAX).collect();
        let compressed = compression.compress(data.clone());
        let decompressed = compression.decompress(compressed).unwrap();
        assert_eq!(decompressed, data);
    }
}
