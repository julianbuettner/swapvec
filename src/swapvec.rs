use std::{
    collections::{hash_map::DefaultHasher, VecDeque},
    fmt::Debug,
    fs::File,
    hash::{Hash, Hasher},
    io::Write,
    os::fd::AsRawFd,
};

use serde::{Deserialize, Serialize};

use crate::{error::SwapVecError, swapveciter::SwapVecIter};

/// Configure compression for the temporary
/// file into which your data might be swapped out.  
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Compression {
    /// Read more about LZ4 here: [LZ4]
    /// [LZ4]: https://github.com/lz4/lz4
    Lz4,
}

/// Configure when and how the vector should swap.
///
/// The file creation will happen after max(`swap_after`, `batch_size`)
/// elements.
///
/// Keep in mind, that if the temporary file exists,
/// after ever `batch_size` elements, at least one write (syscall)
/// will happen.
#[derive(Debug, Clone)]
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

pub struct BatchInfo {
    pub hash: u64,
    pub bytes: usize,
}

pub struct CheckedFile {
    pub file: File,
    pub batch_info: Vec<BatchInfo>,
}

impl CheckedFile {
    fn write_all(&mut self, buffer: &Vec<u8>) -> Result<(), std::io::Error> {
        let mut hasher = DefaultHasher::new();
        buffer.hash(&mut hasher);
        let _res = self.file.write_all(buffer);
        self.batch_info.push(BatchInfo {
            hash: hasher.finish(),
            bytes: buffer.len(),
        });
        self.file.flush()
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
    tempfile: Option<CheckedFile>,
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
            "SwapVec {{elements_in_ram: {}, elements_in_file: {}, filedescriptor: {:#?}}}",
            self.vector.len(),
            self.tempfile.as_ref().map_or(0, |x| x.batch_info.len()) * self.config.batch_size,
            self.tempfile.as_ref().map(|x| x.file.as_raw_fd())
        )
    }
}

impl<T> SwapVec<T>
where
    for<'a> T: Serialize + Deserialize<'a> + Hash,
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

    /// Push a single element  
    /// Might return an error, due to possibly triggered batch flush (IO).
    pub fn push(&mut self, element: T) -> Result<(), SwapVecError> {
        self.vector.push_back(element);
        self.after_push_work()
    }

    /// Check if a file has been created.  
    /// Is false if element count is below `swap_after` and below `batch_size`
    pub fn written_to_file(&self) -> bool {
        self.tempfile.is_some()
    }

    /// Get the file size in bytes of the temporary file.
    /// Might do IO and therefore could return some Result.
    pub fn file_size(&self) -> Option<Result<u64, SwapVecError>> {
        match self.tempfile.as_ref() {
            None => None,
            Some(f) => match f.file.metadata() {
                Err(err) => Some(Err(err.into())),
                Ok(meta) => Some(Ok(meta.len())),
            },
        }
    }

    /// Basically elements pushed // `batch_size`
    pub fn batches_written(&self) -> usize {
        match self.tempfile.as_ref() {
            None => 0,
            Some(f) => f.batch_info.len(),
        }
    }

    fn after_push_work(&mut self) -> Result<(), SwapVecError> {
        if self.vector.len() < self.config.batch_size {
            return Ok(());
        }

        if self.tempfile.is_none() && self.vector.len() < self.config.swap_after {
            return Ok(());
        }

        // Do action
        if self.tempfile.is_none() {
            let tf = tempfile::tempfile()?;
            self.tempfile = Some(CheckedFile {
                file: tf,
                batch_info: Vec::new(),
            })
        }

        let batch: Vec<T> = (0..self.config.batch_size)
            .map(|_| self.vector.pop_front().unwrap())
            .collect::<Vec<_>>();

        let mut batch_hash = DefaultHasher::new();
        batch.iter().for_each(|x| x.hash(&mut batch_hash));

        let buffer = bincode::serialize(&batch)?;
        self.tempfile.as_mut().unwrap().write_all(&buffer)?;
        Ok(())
    }
}

impl<T: Serialize + for<'a> Deserialize<'a> + Hash> IntoIterator for SwapVec<T> {
    type Item = Result<T, SwapVecError>;
    type IntoIter = SwapVecIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        SwapVecIter::new(self.tempfile, self.vector, self.config)
    }
}
