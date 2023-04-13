mod error;
mod swapvec;
mod swapveciter;

pub use self::swapvec::{Compression, SwapVec, SwapVecConfig};
pub use error::SwapVecError;
pub use swapveciter::SwapVecIter;
