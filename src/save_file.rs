use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use sha2::{Digest, Sha256};

pub const SAVE_FILE_SIZE: u64 = 9_134_256;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SaveHash([u8; 32]);

impl SaveHash {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseSaveHashError;

impl fmt::Display for ParseSaveHashError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("expected a 64-character hexadecimal SHA-256 hash")
    }
}

impl Error for ParseSaveHashError {}

impl FromStr for SaveHash {
    type Err = ParseSaveHashError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64 {
            return Err(ParseSaveHashError);
        }

        let mut bytes = [0; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let start = index * 2;
            *byte =
                u8::from_str_radix(&value[start..start + 2], 16).map_err(|_| ParseSaveHashError)?;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Debug for SaveHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for SaveHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveFingerprint {
    pub size: u64,
    pub sha256: SaveHash,
}

#[derive(Debug)]
pub enum SaveFileError {
    Open {
        path: PathBuf,
        source: io::Error,
    },
    Metadata {
        path: PathBuf,
        source: io::Error,
    },
    NotAFile {
        path: PathBuf,
    },
    UnexpectedSize {
        path: PathBuf,
        expected: u64,
        actual: u64,
    },
    Read {
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for SaveFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path, source } => {
                write!(formatter, "failed to open {}: {source}", path.display())
            }
            Self::Metadata { path, source } => {
                write!(formatter, "failed to inspect {}: {source}", path.display())
            }
            Self::NotAFile { path } => {
                write!(formatter, "{} is not a regular file", path.display())
            }
            Self::UnexpectedSize {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "{} has size {actual}, expected {expected}",
                path.display()
            ),
            Self::Read { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
        }
    }
}

impl Error for SaveFileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Open { source, .. }
            | Self::Metadata { source, .. }
            | Self::Read { source, .. } => Some(source),
            Self::NotAFile { .. } | Self::UnexpectedSize { .. } => None,
        }
    }
}

pub fn validate_and_fingerprint(path: &Path) -> Result<SaveFingerprint, SaveFileError> {
    let metadata = fs::metadata(path).map_err(|source| SaveFileError::Metadata {
        path: path.to_path_buf(),
        source,
    })?;

    if !metadata.is_file() {
        return Err(SaveFileError::NotAFile {
            path: path.to_path_buf(),
        });
    }

    if metadata.len() != SAVE_FILE_SIZE {
        return Err(SaveFileError::UnexpectedSize {
            path: path.to_path_buf(),
            expected: SAVE_FILE_SIZE,
            actual: metadata.len(),
        });
    }

    let file = File::open(path).map_err(|source| SaveFileError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    let fingerprint =
        fingerprint_reader(BufReader::new(file)).map_err(|source| SaveFileError::Read {
            path: path.to_path_buf(),
            source,
        })?;

    if fingerprint.size != SAVE_FILE_SIZE {
        return Err(SaveFileError::UnexpectedSize {
            path: path.to_path_buf(),
            expected: SAVE_FILE_SIZE,
            actual: fingerprint.size,
        });
    }

    Ok(fingerprint)
}

pub(crate) fn fingerprint_reader(mut reader: impl Read) -> io::Result<SaveFingerprint> {
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut bytes_read = 0;

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        bytes_read += read as u64;
    }

    Ok(SaveFingerprint {
        size: bytes_read,
        sha256: SaveHash(hasher.finalize().into()),
    })
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::{Cursor, Seek, SeekFrom, Write};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn hashes_a_known_sha256_test_vector() {
        let fingerprint = fingerprint_reader(Cursor::new(b"abc")).unwrap();

        assert_eq!(3, fingerprint.size);
        assert_eq!(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            fingerprint.sha256.to_string()
        );
    }

    #[test]
    fn parses_a_displayed_hash() {
        let expected = SaveHash::from_bytes([0xab; 32]);

        let parsed = expected.to_string().parse::<SaveHash>().unwrap();

        assert_eq!(expected, parsed);
        assert!("invalid".parse::<SaveHash>().is_err());
    }

    #[test]
    fn rejects_a_file_with_an_unexpected_size() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("small.dat");
        fs::write(&path, b"not a Mirror's Edge save").unwrap();

        let error = validate_and_fingerprint(&path).unwrap_err();

        assert!(matches!(
            error,
            SaveFileError::UnexpectedSize {
                expected: SAVE_FILE_SIZE,
                actual: 24,
                ..
            }
        ));
    }

    #[test]
    fn validates_and_hashes_a_fixed_size_file() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("valid.dat");
        let mut file = File::create(&path).unwrap();
        file.set_len(SAVE_FILE_SIZE).unwrap();
        file.seek(SeekFrom::Start(SAVE_FILE_SIZE - 1)).unwrap();
        file.write_all(&[1]).unwrap();
        file.flush().unwrap();

        let fingerprint = validate_and_fingerprint(&path).unwrap();

        assert_eq!(SAVE_FILE_SIZE, fingerprint.size);
        assert_eq!(32, fingerprint.sha256.as_bytes().len());
    }

    #[test]
    fn rejects_a_directory() {
        let directory = TempDir::new().unwrap();

        let error = validate_and_fingerprint(directory.path()).unwrap_err();

        assert!(matches!(error, SaveFileError::NotAFile { .. }));
    }
}
