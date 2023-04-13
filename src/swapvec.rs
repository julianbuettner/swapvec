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

#[derive(Debug, Clone)]
pub enum Compression {
    Lz4,
}

#[derive(Debug, Clone)]
pub struct SwapVecConfig {
    pub swap_after: usize,
    pub batch_size: usize,
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
            self.tempfile
                .as_ref()
                .map(|x| x.batch_info.len())
                .unwrap_or(0)
                * self.config.batch_size,
            self.tempfile.as_ref().map(|x| x.file.as_raw_fd())
        )
    }
}

impl<T> SwapVec<T>
where
    for<'a> T: Serialize + Deserialize<'a> + Hash,
{
    pub fn with_config(config: SwapVecConfig) -> Self {
        Self {
            tempfile: None,
            vector: VecDeque::new(),
            config,
        }
    }

    pub fn consume(&mut self, it: impl Iterator<Item = T>) -> Result<(), SwapVecError> {
        for element in it {
            self.push(element)?;
            self.after_push_work()?;
        }
        Ok(())
    }

    pub fn push(&mut self, element: T) -> Result<(), SwapVecError> {
        self.vector.push_back(element);
        self.after_push_work()
    }

    pub fn written_to_file(&self) -> bool {
        self.tempfile.is_some()
    }

    pub fn file_size(&self) -> Option<Result<u64, SwapVecError>> {
        match self.tempfile.as_ref() {
            None => None,
            Some(f) => match f.file.metadata() {
                Err(err) => Some(Err(err.into())),
                Ok(meta) => Some(Ok(meta.len())),
            },
        }
    }

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
