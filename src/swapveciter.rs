use std::collections::hash_map::DefaultHasher;

use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::{collections::VecDeque, io::Seek};

use serde::{Deserialize, Serialize};

use crate::error::SwapVecError;
use crate::swapvec::{BatchInfo, CheckedFile, SwapVecConfig};

pub struct CheckedFileRead {
    pub file: File,
    pub batch_info_rev: Vec<BatchInfo>,
}

/// Iterator for SwapVec.
///
/// Items might be read from disk,
/// so every item is wrapped in a `Result`.  
/// The iterator aborts after the first error.
///
/// Dropping the iterator removes the temporary file, if existing.  
/// Also quitting the program should remove the temporary file.
pub struct SwapVecIter<T>
where
    for<'a> T: Serialize + Deserialize<'a> + Hash,
{
    seeked_zero: bool,
    current_batch_rev: Vec<T>,
    tempfile: Option<CheckedFileRead>,
    // last_elements are elements,
    // which have not been written to disk.
    // Therefore, for iterating from zero,
    // first read elements from disk and
    // then from last_elements.
    last_elements: VecDeque<T>,
}

impl<T: Serialize + for<'a> Deserialize<'a> + Hash> SwapVecIter<T> {
    /// This method should not even be public,
    /// but I don't know how to make it private.
    pub fn new(
        tempfile_written: Option<CheckedFile>,
        last_elements: VecDeque<T>,
        config: SwapVecConfig,
    ) -> Self {
        let tempfile = tempfile_written.map(|mut x| {
            x.batch_info.reverse();
            CheckedFileRead {
                file: x.file,
                batch_info_rev: x.batch_info,
            }
        });
        Self {
            seeked_zero: false,
            current_batch_rev: Vec::with_capacity(config.batch_size),
            last_elements,
            tempfile,
        }
    }

    fn ensure_seeked_zero(&mut self) -> Result<(), SwapVecError> {
        if !self.seeked_zero {
            if let Some(some_tempfile) = self.tempfile.as_mut() {
                if let Err(err) = some_tempfile.file.seek(std::io::SeekFrom::Start(0)) {
                    return Err(err.into());
                }
            }
            self.seeked_zero = true;
        }
        Ok(())
    }

    fn read_batch(&mut self) -> Result<Option<Vec<T>>, SwapVecError> {
        if self.tempfile.is_none() {
            return Ok(None);
        }
        self.ensure_seeked_zero()?;

        let tempfile = self.tempfile.as_mut().unwrap();
        let batch_info = tempfile.batch_info_rev.pop();
        if batch_info.is_none() {
            return Ok(None);
        }

        let batch_info = batch_info.unwrap();
        let mut buffer = vec![0; batch_info.bytes];
        tempfile.file.read_exact(&mut buffer)?;

        let mut hasher = DefaultHasher::new();
        buffer.hash(&mut hasher);
        if hasher.finish() != batch_info.hash {
            return Err(SwapVecError::WrongChecksum);
        }

        let batch: Vec<T> = bincode::deserialize(&buffer)?;

        // If everything from file has been read,
        // mark as empty.
        if tempfile.batch_info_rev.is_empty() {
            self.tempfile = None;
        }

        Ok(Some(batch))
    }

    fn next_in_batch(&mut self) -> Result<Option<T>, SwapVecError> {
        if let Some(v) = self.current_batch_rev.pop() {
            return Ok(Some(v));
        }
        if let Some(mut new_batch) = self.read_batch()? {
            new_batch.reverse();
            self.current_batch_rev = new_batch;
            Ok(self.current_batch_rev.pop())
        } else {
            Ok(None)
        }
    }
}

impl<T: Serialize + for<'a> Deserialize<'a> + Hash> Iterator for SwapVecIter<T> {
    type Item = Result<T, SwapVecError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.current_batch_rev.pop() {
            return Some(Ok(item));
        }

        let next_in_batch = self.next_in_batch();
        if let Err(err) = next_in_batch {
            return Some(Err(err));
        }
        if let Ok(Some(item)) = next_in_batch {
            return Some(Ok(item));
        }

        // File has been exhausted.
        self.last_elements.pop_front().map(|x| Ok(x))
    }
}
