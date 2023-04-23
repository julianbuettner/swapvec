use lz4_flex::{compress_prepend_size, decompress_size_prepended};

use crate::{swapvec::CompressionLevel, Compression};

pub trait Compress {
    fn compress(&self, block: Vec<u8>) -> Vec<u8>;
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
            None => block,
        }
    }
    fn decompress(&self, block: Vec<u8>) -> Result<Vec<u8>, ()> {
        match self {
            Some(Compression::Lz4) => decompress_size_prepended(&block).map_err(|_| ()),
            Some(Compression::Deflate(_)) => {
                miniz_oxide::inflate::decompress_to_vec(&block).map_err(|_| ())
            }
            None => Ok(block),
        }
    }
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
