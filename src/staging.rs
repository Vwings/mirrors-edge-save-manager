use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::save_file::SaveFingerprint;
use crate::storage::{StorageError, StoredSaveRepository};

const REPLACEMENT_PREFIX: &str = ".mirrors-edge-save-manager-";
const REPLACEMENT_SUFFIX: &str = ".replacement.dat";

#[derive(Debug)]
pub enum StagingError {
    InvalidTransactionId(String),
    CurrentWithoutParent(PathBuf),
    Storage(StorageError),
}

impl fmt::Display for StagingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransactionId(id) => {
                write!(formatter, "transaction ID is not a UUID: {id}")
            }
            Self::CurrentWithoutParent(path) => {
                write!(
                    formatter,
                    "Current path has no parent directory: {}",
                    path.display()
                )
            }
            Self::Storage(source) => write!(formatter, "failed to stage StoredSave: {source}"),
        }
    }
}

impl Error for StagingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(source) => Some(source),
            Self::InvalidTransactionId(_) | Self::CurrentWithoutParent(_) => None,
        }
    }
}

impl From<StorageError> for StagingError {
    fn from(source: StorageError) -> Self {
        Self::Storage(source)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedReplacement {
    pub path: PathBuf,
    pub fingerprint: SaveFingerprint,
}

pub fn stage_stored_save(
    repository: &StoredSaveRepository,
    stored_save_id: &str,
    current_path: &Path,
    transaction_id: &str,
) -> Result<StagedReplacement, StagingError> {
    let transaction_id = Uuid::parse_str(transaction_id)
        .map_err(|_| StagingError::InvalidTransactionId(transaction_id.into()))?;
    let current_directory = current_path
        .parent()
        .ok_or_else(|| StagingError::CurrentWithoutParent(current_path.to_path_buf()))?;
    let path = current_directory.join(format!(
        "{REPLACEMENT_PREFIX}{transaction_id}{REPLACEMENT_SUFFIX}"
    ));
    let fingerprint = repository.materialize_payload(stored_save_id, &path)?;

    Ok(StagedReplacement { path, fingerprint })
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::{Seek, SeekFrom, Write};

    use tempfile::TempDir;

    use crate::save_file::{SAVE_FILE_SIZE, validate_and_fingerprint};
    use crate::stored_save::{StoredSaveKind, StoredSaveOrigin};

    use super::*;

    fn create_valid_save(directory: &Path, name: &str) -> PathBuf {
        let path = directory.join(name);
        let mut file = File::create(&path).unwrap();
        file.set_len(SAVE_FILE_SIZE).unwrap();
        file.seek(SeekFrom::Start(SAVE_FILE_SIZE - 1)).unwrap();
        file.write_all(&[1]).unwrap();
        file.flush().unwrap();
        path
    }

    fn capture_save(directory: &TempDir) -> (StoredSaveRepository, String, PathBuf) {
        let source = create_valid_save(directory.path(), "source.dat");
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let captured = repository
            .capture(
                &source,
                StoredSaveKind::Preset,
                "Practice".into(),
                None,
                StoredSaveOrigin::Imported,
            )
            .unwrap();
        (repository, captured.metadata.id, source)
    }

    #[test]
    fn stages_a_verified_payload_beside_current() {
        let directory = TempDir::new().unwrap();
        let (repository, stored_save_id, source) = capture_save(&directory);
        let save_directory = directory.path().join("game-save");
        fs::create_dir(&save_directory).unwrap();
        let current = create_valid_save(&save_directory, "Vwings.dat");
        let current_fingerprint = validate_and_fingerprint(&current).unwrap();
        let transaction_id = Uuid::new_v4().to_string();

        let staged =
            stage_stored_save(&repository, &stored_save_id, &current, &transaction_id).unwrap();

        assert_eq!(save_directory, staged.path.parent().unwrap());
        assert_eq!(
            format!("{REPLACEMENT_PREFIX}{transaction_id}{REPLACEMENT_SUFFIX}"),
            staged.path.file_name().unwrap().to_string_lossy()
        );
        assert_eq!(
            staged.fingerprint,
            validate_and_fingerprint(&source).unwrap()
        );
        assert_eq!(
            staged.fingerprint,
            validate_and_fingerprint(&staged.path).unwrap()
        );
        assert_eq!(
            current_fingerprint,
            validate_and_fingerprint(&current).unwrap()
        );
    }

    #[test]
    fn does_not_overwrite_an_existing_staging_path() {
        let directory = TempDir::new().unwrap();
        let (repository, stored_save_id, _) = capture_save(&directory);
        let current = directory.path().join("Vwings.dat");
        let transaction_id = Uuid::new_v4().to_string();
        let staged_path = directory.path().join(format!(
            "{REPLACEMENT_PREFIX}{transaction_id}{REPLACEMENT_SUFFIX}"
        ));
        fs::write(&staged_path, b"keep this file").unwrap();

        let result = stage_stored_save(&repository, &stored_save_id, &current, &transaction_id);

        assert!(matches!(
            result,
            Err(StagingError::Storage(StorageError::Io { .. }))
        ));
        assert_eq!(b"keep this file", fs::read(staged_path).unwrap().as_slice());
    }

    #[test]
    fn removes_a_partial_file_when_payload_decompression_fails() {
        let directory = TempDir::new().unwrap();
        let (repository, stored_save_id, _) = capture_save(&directory);
        let payload = repository
            .root()
            .join("stored-saves")
            .join(&stored_save_id)
            .join("payload.dat.gz");
        fs::write(payload, b"not gzip").unwrap();
        let current = directory.path().join("Vwings.dat");
        let transaction_id = Uuid::new_v4().to_string();
        let staged_path = directory.path().join(format!(
            "{REPLACEMENT_PREFIX}{transaction_id}{REPLACEMENT_SUFFIX}"
        ));

        let result = stage_stored_save(&repository, &stored_save_id, &current, &transaction_id);

        assert!(result.is_err());
        assert!(!staged_path.exists());
    }

    #[test]
    fn rejects_an_invalid_transaction_id_before_creating_a_file() {
        let directory = TempDir::new().unwrap();
        let (repository, stored_save_id, _) = capture_save(&directory);
        let current = directory.path().join("Vwings.dat");
        let entry_count = fs::read_dir(directory.path()).unwrap().count();

        let result = stage_stored_save(&repository, &stored_save_id, &current, "not-a-uuid");

        assert!(matches!(result, Err(StagingError::InvalidTransactionId(_))));
        assert_eq!(entry_count, fs::read_dir(directory.path()).unwrap().count());
    }
}
