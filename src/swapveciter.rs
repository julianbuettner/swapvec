use std::collections::VecDeque;
use std::fs::File;

use serde::{Deserialize, Serialize};

use crate::checkedfile::{BatchReader, BatchWriter};
use crate::compression::Compress;
use crate::error::SwapVecError;
use crate::swapvec::SwapVecConfig;

struct VecDequeIndex<T: Clone> {
    value: VecDeque<T>,
}

impl<T: Clone> From<VecDeque<T>> for VecDequeIndex<T> {
    fn from(value: VecDeque<T>) -> Self {
        Self { value }
    }
}

impl<T: Clone> VecDequeIndex<T> {
    fn get(&self, i: usize) -> Option<T> {
        let (a, b) = self.value.as_slices();
        if i < a.len() {
            a.get(i).cloned()
        } else {
            b.get(i - a.len()).cloned()
        }
    }
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
    for<'a> T: Serialize + Deserialize<'a> + Clone,
{
    // Do not error on new, because into_iter()
    // is not allowed to fail. Fail at first try then.
    new_error: Option<std::io::Error>,
    current_batch_rev: Vec<T>,
    tempfile: Option<BatchReader<File>>,
    // last_elements are elements,
    // which have not been written to disk.
    // Therefore, for iterating from zero,
    // first read elements from disk and
    // then from last_elements.
    last_elements: VecDequeIndex<T>,
    last_elements_index: usize,
    config: SwapVecConfig,
}

impl<T: Serialize + for<'a> Deserialize<'a> + Clone> SwapVecIter<T> {
    pub(crate) fn new(
        tempfile_written: Option<BatchWriter<File>>,
        last_elements: VecDeque<T>,
        config: SwapVecConfig,
    ) -> Self {
        let (tempfile, new_error) = match tempfile_written.map(|v| v.try_into()) {
            None => (None, None),
            Some(Ok(v)) => (Some(v), None),
            Some(Err(e)) => (None, Some(e)),
        };

        let last_elements: VecDequeIndex<_> = last_elements.into();
        Self {
            new_error,
            current_batch_rev: Vec::with_capacity(config.batch_size),
            last_elements,
            last_elements_index: 0,
            tempfile,
            config,
        }
    }

    fn read_batch(&mut self) -> Result<Option<Vec<T>>, SwapVecError> {
        if self.tempfile.is_none() {
            return Ok(None);
        }
        assert!(self.tempfile.is_some());
        if let Some(err) = self.new_error.take() {
            return Err(err.into());
        }

        let tempfile = self.tempfile.as_mut().unwrap();
        let buffer = tempfile.read_batch()?;
        if buffer.is_none() {
            return Ok(None);
        }
        let buffer = buffer.unwrap();
        let decompressed: Vec<u8> = self
            .config
            .compression
            .decompress(buffer.to_vec())
            .map_err(|_| SwapVecError::Decompression)?;

        let batch: Vec<T> = bincode::deserialize(&decompressed)?;

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

    /// Resets the iteration, starting from the first element.
    /// If a file exists, it will be read from the beginning.  
    ///
    /// To use this feature, you probably don't want to consume
    /// the iterator (`bigvec.map(|x| x * 2)`), but to use
    /// [`Iterator::by_ref()`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.by_ref)
    /// ```rust
    /// let mut bigvec = swapvec::SwapVec::default();
    /// bigvec.consume(0..99);
    /// let mut new_iterator = bigvec.into_iter();
    /// let sum: usize = new_iterator.by_ref().map(|v| v.unwrap()).sum();
    /// new_iterator.reset();
    /// let sum_double: usize = new_iterator.by_ref().map(|v| v.unwrap() * 2).sum();
    /// ```
    pub fn reset(&mut self) {
        self.current_batch_rev.clear();
        self.last_elements_index = 0;
        if let Some(tempfile) = self.tempfile.as_mut() {
            if let Err(e) = tempfile.reset() {
                self.new_error = Some(e);
            }
        }
    }
}

impl<T: Serialize + for<'a> Deserialize<'a> + Clone> Iterator for SwapVecIter<T> {
    type Item = Result<T, SwapVecError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.current_batch_rev.pop() {
            return Some(Ok(item));
        }

        match self.next_in_batch() {
            Err(err) => Some(Err(err)),
            Ok(Some(item)) => Some(Ok(item)),
            Ok(None) => {
                let index = self.last_elements_index;
                self.last_elements_index += 1;
                self.last_elements.get(index).map(|x| Ok(x))
            }
        }
    }
}
