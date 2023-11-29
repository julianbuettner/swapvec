use std::{
    hash::{Hash, Hasher},
    io::{self, BufReader, BufWriter, Error, Read, Seek, Write}, collections::hash_map::DefaultHasher,
};

use crate::SwapVecError;

#[derive(Debug)]
pub struct BatchInfo {
    pub hash: u64,
    pub bytes: usize,
}

pub(crate) struct BatchWriter<T: Write> {
    inner: BufWriter<T>,
    batch_infos: Vec<BatchInfo>,
}

pub(crate) struct BatchReader<T: Read> {
    inner: BufReader<T>,
    batch_infos: Vec<BatchInfo>,
    batch_index: usize,
    buffer: Vec<u8>,
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

impl<T: Write> BatchWriter<T> {
    pub fn new(writer: T) -> Self {
        Self {
            batch_infos: Vec::new(),
            inner: BufWriter::new(writer),
        }
    }
    pub fn write_batch(&mut self, buffer: &[u8]) -> Result<(), io::Error> {
        self.inner.write_all(buffer)?;
        self.batch_infos.push(BatchInfo {
            hash: hash_bytes(buffer),
            bytes: buffer.len(),
        });
        self.inner.flush()
    }
    pub fn bytes_written(&self) -> usize {
        self.batch_infos.iter().map(|b| b.bytes).sum()
    }
    pub fn batch_count(&self) -> usize {
        self.batch_infos.len()
    }
}

impl<T: Read + Seek> BatchReader<T> {
    pub fn reset(&mut self) -> Result<(), Error> {
        self.inner.seek(io::SeekFrom::Start(0))?;
        self.batch_index = 0;
        self.buffer.clear();
        Ok(())
    }
}

impl<T: Read> BatchReader<T> {
    pub fn read_batch(&mut self) -> Result<Option<&[u8]>, SwapVecError> {
        let batch_info = self.batch_infos.get(self.batch_index);
        self.batch_index += 1;
        if batch_info.is_none() {
            return Ok(None);
        }
        let batch_info = batch_info.unwrap();
        self.buffer.resize(batch_info.bytes, 0);
        self.inner.read_exact(self.buffer.as_mut_slice())?;
        if hash_bytes(self.buffer.as_slice()) != batch_info.hash {
            // return Err(SwapVecError::WrongChecksum);
        }
        Ok(Some(self.buffer.as_slice()))
    }
}

impl<T: Read + Write + Seek> TryFrom<BatchWriter<T>> for BatchReader<T> {
    type Error = std::io::Error;

    fn try_from(value: BatchWriter<T>) -> Result<Self, Self::Error> {
        let mut inner = value
            .inner
            .into_inner()
            .map_err(|inner_error| inner_error.into_error())?;
        inner.seek(io::SeekFrom::Start(0))?;
        Ok(Self {
            inner: BufReader::new(inner),
            batch_infos: value.batch_infos,
            batch_index: 0,
            buffer: Vec::new(),
        })
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn read_write_checked_io() {
        let buffer = Cursor::new(vec![0; 128]);
        let mut batch_writer = BatchWriter::new(buffer);
        batch_writer
            .write_batch(&[1, 2, 3])
            .expect("Could not write to IO buffer");
        batch_writer
            .write_batch(&[44, 55])
            .expect("Could not write to IO buffer");

        // batch_writer.wtf();
        // panic!()
        let mut reader: BatchReader<_> = batch_writer
            .try_into()
            .expect("Could not flush into IO buffer");
        assert_eq!(
            reader
                .read_batch()
                .expect("Could not read batch")
                .expect("Batch was unexpectedly empty"),
            &[1, 2, 3]
        );
        reader.reset().expect("Could not reset");
        assert_eq!(
            reader
                .read_batch()
                .expect("Could not read batch")
                .expect("Batch was unexpectedly empty"),
            &[1, 2, 3]
        );
        assert_eq!(
            reader
                .read_batch()
                .expect("Could not read batch")
                .expect("Batch was unexpectedly empty"),
            &[44, 55]
        );
    }
}
