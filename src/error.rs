/// A collection of all possible errors.
///
/// Errors could be divided into write and read
/// errors, but this makes error handling a bit less
/// comfortable, so they are united here.
#[derive(Debug)]
#[non_exhaustive]
pub enum SwapVecError {
    /// The program is missing permissions to create a temporary file
    MissingPermissions,
    /// A batch could not be written due to a full disk
    OutOfDisk,
    /// A read back batch had a wrong checksum
    WrongChecksum,
    /// A batch could not be decompressed correctly.
    /// This also happens only if the file has been corrupted.
    Decompression,
    /// The batch was read back successfully,
    /// but the serialization failed.
    ///
    /// Take a look at the `Serialize` implementation
    /// of your type `T`.
    SerializationFailed(bincode::ErrorKind),
    /// Every other possibility
    Other,
}

impl From<std::io::Error> for SwapVecError {
    fn from(_value: std::io::Error) -> Self {
        match _value.kind() {
            // TODO https://github.com/rust-lang/rust/issues/86442
            // std::io::ErrorKind::StorageFull => Self::OutOfDisk,
            std::io::ErrorKind::PermissionDenied => Self::MissingPermissions,
            _ => Self::Other,
        }
    }
}

impl From<Box<bincode::ErrorKind>> for SwapVecError {
    fn from(value: Box<bincode::ErrorKind>) -> Self {
        SwapVecError::SerializationFailed(*value)
    }
}
