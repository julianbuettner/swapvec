#[derive(Debug)]
pub enum SwapVecError {
    MissingPermissions,
    OutOfDisk,
    WrongChecksum,
    SerializationFailed(bincode::ErrorKind),
    Other,
}

impl From<std::io::Error> for SwapVecError {
    fn from(_value: std::io::Error) -> Self {
        todo!()
    }
}

impl From<Box<bincode::ErrorKind>> for SwapVecError {
    fn from(value: Box<bincode::ErrorKind>) -> Self {
        SwapVecError::SerializationFailed(*value)
    }
}
