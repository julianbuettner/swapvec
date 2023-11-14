use std::{
    collections::VecDeque,
    fmt::Debug,
    fs::File,
};

use serde::{Deserialize, Serialize};

use crate::{
    checkedfile::BatchWriter,
    compression::{Compress, CompressBoxedClone},
    error::SwapVecError,
    swapveciter::SwapVecIter,
};

/// Set compression level of the compression
/// algorithm. This maps to different values
/// depending on the chosen algortihm.
#[derive(Clone, Debug, Copy)]
pub enum CompressionLevel {
    /// Slower than default, higher compression.
    /// Might be useful for big amount of data
    /// which requires heavier compression.
    Slow,
    /// A good ratio of compression ratio to cpu time.
    Default,
    /// Accept worse compression for speed.
    /// Useful for easily compressable data with
    /// many repetitions.
    Fast,
}

/// Configure compression for the temporary
/// file into which your data might be swapped out.  
#[derive(Debug)]
#[non_exhaustive]
pub enum Compression {
    /// Read more about LZ4 here: [LZ4]
    /// [LZ4]: https://github.com/lz4/lz4
    Lz4,
    /// Deflate, mostly known from gzip.
    Deflate(CompressionLevel),
    /// Provide your own compression algortihm by implementing
    /// `Compress`.
    Custom(Box<dyn CompressBoxedClone>),
}

impl Clone for Compression {
    fn clone(&self) -> Self {
        match &self {
            Self::Lz4 => Self::Lz4,
            Self::Deflate(n) => Self::Deflate(*n),
            Self::Custom(x) => Self::Custom(x.boxed_clone()),
        }
    }
}

/// Configure when and how the vector should swap.
///
/// The file creation will happen after max(swap_after, batch_size)
/// elements.
///
/// Keep in mind, that if the temporary file exists,
/// after ever batch_size elements, at least one write (syscall)
/// will happen.
#[derive(Debug)]
pub struct SwapVecConfig {
    /// The vector will create a temporary file and starting to
    /// swap after so many elements.
    /// If your elements have a certain size in bytes, you can
    /// multiply this value to calculate the required storage.
    ///
    /// If you want to start swapping with the first batch,
    /// set to batch_size or smaller.
    ///
    /// Default: 32 * 1024 * 1024
    pub swap_after: usize,
    /// How many elements at once should be written to disk.  
    /// Keep in mind, that for every batch one hash (`u64`)
    /// and one bytecount (`usize`)
    /// will be kept in memory.
    ///
    /// One batch write will result in at least one syscall.
    ///
    /// Default: 32 * 1024
    pub batch_size: usize,
    /// If and how you want to compress your temporary file.  
    /// This might be only useful for data which is compressable,
    /// like timeseries often are.
    ///
    /// Default: No compression
    pub compression: Option<Compression>,
}

impl Default for SwapVecConfig {
    fn default() -> Self {
        Self {
            swap_after: 32 * 1024 * 1024,
            batch_size: 32 * 1024,
            compression: None,
        }
    }
}

/// An only growing array type
/// which swaps to disk, based on it's initial configuration.
///
/// Create a mutable instance, and then
/// pass iterators or elements to grow it.
/// ```rust
/// let mut bigvec = swapvec::SwapVec::default();
/// let iterator = (0..9);
/// bigvec.consume(iterator);
/// bigvec.push(99);
/// let new_iterator = bigvec.into_iter();
/// ```
pub struct SwapVec<T>
where
    for<'a> T: Serialize + Deserialize<'a>,
{
    tempfile: Option<BatchWriter<File>>,
    vector: VecDeque<T>,
    config: SwapVecConfig,
}

impl<T: Serialize + for<'a> Deserialize<'a>> Default for SwapVec<T> {
    fn default() -> Self {
        Self {
            tempfile: None,
            vector: VecDeque::new(),
            config: SwapVecConfig::default(),
        }
    }
}

impl<T: Serialize + for<'a> Deserialize<'a>> Debug for SwapVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SwapVec {{elements_in_ram: {}, elements_in_file: {}}}",
            self.vector.len(),
            self.tempfile.as_ref().map(|x| x.batch_count()).unwrap_or(0) * self.config.batch_size,
        )
    }
}

impl<T> SwapVec<T>
where
    for<'a> T: Serialize + Deserialize<'a> + Clone,
{
    /// Intialize with non-default configuration.
    pub fn with_config(config: SwapVecConfig) -> Self {
        Self {
            tempfile: None,
            vector: VecDeque::new(),
            config,
        }
    }

    /// Give away an entire iterator for consumption.  
    /// Might return an error, due to possibly triggered batch flush (IO).
    pub fn consume(&mut self, it: impl Iterator<Item = T>) -> Result<(), SwapVecError> {
        for element in it {
            self.push(element)?;
            self.after_push_work()?;
        }
        Ok(())
    }

    /// Push a single element.
    /// Might return an error, due to possibly triggered batch flush (IO).
    /// Will write at most one batch per insert.
    /// If `swap_after` is bigger than `batch_size` and a file is created,
    /// every insert will
    /// write one batch to disk, until the elements in memory have a count
    /// smaller than or equal to batch size.
    pub fn push(&mut self, element: T) -> Result<(), SwapVecError> {
        self.vector.push_back(element);
        self.after_push_work()
    }

    /// Check if enough items have been pushed so that
    /// the temporary file has been created.  
    /// Will be false if element count is below swap_after and below batch_size
    pub fn written_to_file(&self) -> bool {
        self.tempfile.is_some()
    }

    /// Get the file size in bytes of the temporary file.
    /// Might do IO and therefore could return some Result.
    pub fn file_size(&self) -> Option<usize> {
        self.tempfile.as_ref().map(|f| f.bytes_written())
    }

    /// Basically int(elements pushed / batch size)
    pub fn batches_written(&self) -> usize {
        match self.tempfile.as_ref() {
            None => 0,
            Some(f) => f.batch_count(),
        }
    }

    fn after_push_work(&mut self) -> Result<(), SwapVecError> {
        if self.vector.len() <= self.config.batch_size {
            return Ok(());
        }
        if self.tempfile.is_none() && self.vector.len() <= self.config.swap_after {
            return Ok(());
        }

        // Flush batch
        if self.tempfile.is_none() {
            let tf = tempfile::tempfile()?;
            self.tempfile = Some(BatchWriter::new(tf));
        }
        assert!(self.tempfile.is_some());
        let batch: Vec<_> = self.vector.drain(0..self.config.batch_size).collect();

        let buffer = bincode::serialize(&batch)?;
        let compressed = self.config.compression.compress(buffer);
        self.tempfile.as_mut().unwrap().write_batch(&compressed)?;
        Ok(())
    }
}

impl<T: Serialize + for<'a> Deserialize<'a> + Clone> IntoIterator for SwapVec<T> {
    type Item = Result<T, SwapVecError>;
    type IntoIter = SwapVecIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        SwapVecIter::new(self.tempfile, self.vector, self.config)
    }
}
