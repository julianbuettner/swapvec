#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

mod compression;
mod error;
mod swapvec;
mod swapveciter;
mod checkedfile;

pub use self::swapvec::{Compression, CompressionLevel, SwapVec, SwapVecConfig};
pub use compression::{Compress, CompressBoxedClone};
pub use error::SwapVecError;
pub use swapveciter::SwapVecIter;
