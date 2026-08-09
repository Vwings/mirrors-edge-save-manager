use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::alias::{AliasError, resolve_current_alias};
use crate::discovery::{self, CurrentSaveDiscovery, DiscoveryError};
use crate::known_folders;
use crate::mutation_guard::{MutationGuard, MutationGuardError};
use crate::recovery::{RecoveryError, unfinished_journals};
use crate::storage::{CaptureResult, StorageError, StoredSaveRepository};
use crate::stored_save::{StoredSaveKind, StoredSaveOrigin};

pub struct CaptureCurrentRequest {
    pub alias: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug)]
pub enum CaptureCurrentError {
    Alias(AliasError),
    MutationGuard(MutationGuardError),
    Recovery(RecoveryError),
    RecoveryRequired(Vec<PathBuf>),
    Discovery(DiscoveryError),
    SaveDirectoryMissing(PathBuf),
    CurrentMissing(PathBuf),
    Storage(StorageError),
}

impl fmt::Display for CaptureCurrentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alias(source) => write!(formatter, "invalid alias: {source}"),
            Self::MutationGuard(source) => write!(formatter, "capture is blocked: {source}"),
            Self::Recovery(source) => write!(formatter, "transaction scan failed: {source}"),
            Self::RecoveryRequired(paths) => write!(
                formatter,
                "unfinished transaction recovery is required before capture: {paths:?}"
            ),
            Self::Discovery(source) => write!(formatter, "Current discovery failed: {source}"),
            Self::SaveDirectoryMissing(path) => {
                write!(
                    formatter,
                    "save directory does not exist: {}",
                    path.display()
                )
            }
            Self::CurrentMissing(path) => {
                write!(formatter, "Current save is missing from {}", path.display())
            }
            Self::Storage(source) => write!(formatter, "Current capture failed: {source}"),
        }
    }
}

impl Error for CaptureCurrentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Alias(source) => Some(source),
            Self::MutationGuard(source) => Some(source),
            Self::Recovery(source) => Some(source),
            Self::Discovery(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::RecoveryRequired(_) | Self::SaveDirectoryMissing(_) | Self::CurrentMissing(_) => {
                None
            }
        }
    }
}

pub fn capture_current_as_stash(
    repository: &StoredSaveRepository,
    request: CaptureCurrentRequest,
) -> Result<CaptureResult, CaptureCurrentError> {
    let documents = known_folders::documents()
        .map_err(|source| CaptureCurrentError::Discovery(DiscoveryError::KnownFolder(source)))?;
    capture_current_as_stash_in_documents(repository, &documents, request)
}

pub fn capture_current_as_stash_in_documents(
    repository: &StoredSaveRepository,
    documents_directory: &Path,
    request: CaptureCurrentRequest,
) -> Result<CaptureResult, CaptureCurrentError> {
    capture_current_in_documents(
        repository,
        documents_directory,
        request,
        StoredSaveKind::Stash,
    )
}

pub fn capture_current_as_preset(
    repository: &StoredSaveRepository,
    request: CaptureCurrentRequest,
) -> Result<CaptureResult, CaptureCurrentError> {
    let documents = known_folders::documents()
        .map_err(|source| CaptureCurrentError::Discovery(DiscoveryError::KnownFolder(source)))?;
    capture_current_as_preset_in_documents(repository, &documents, request)
}

pub fn capture_current_as_preset_in_documents(
    repository: &StoredSaveRepository,
    documents_directory: &Path,
    request: CaptureCurrentRequest,
) -> Result<CaptureResult, CaptureCurrentError> {
    capture_current_in_documents(
        repository,
        documents_directory,
        request,
        StoredSaveKind::Preset,
    )
}

fn capture_current_in_documents(
    repository: &StoredSaveRepository,
    documents_directory: &Path,
    request: CaptureCurrentRequest,
    kind: StoredSaveKind,
) -> Result<CaptureResult, CaptureCurrentError> {
    let _guard = MutationGuard::acquire().map_err(CaptureCurrentError::MutationGuard)?;
    let unfinished =
        unfinished_journals(repository.root()).map_err(CaptureCurrentError::Recovery)?;
    if !unfinished.is_empty() {
        return Err(CaptureCurrentError::RecoveryRequired(unfinished));
    }
    let current = require_current(discovery::discover_current_in_documents(
        documents_directory,
    )?)?;
    let classification = match kind {
        StoredSaveKind::Preset => "Preset",
        StoredSaveKind::Stash => "Stash",
    };
    let alias =
        resolve_current_alias(request.alias, classification).map_err(CaptureCurrentError::Alias)?;
    repository
        .capture(
            current.path(),
            kind,
            alias,
            request.description,
            StoredSaveOrigin::Current,
        )
        .map_err(CaptureCurrentError::Storage)
}

fn require_current(
    discovery: CurrentSaveDiscovery,
) -> Result<discovery::CurrentSave, CaptureCurrentError> {
    match discovery {
        CurrentSaveDiscovery::CurrentFound(current) => Ok(current),
        CurrentSaveDiscovery::SaveDirectoryMissing { directory } => {
            Err(CaptureCurrentError::SaveDirectoryMissing(directory))
        }
        CurrentSaveDiscovery::CurrentMissing { directory } => {
            Err(CaptureCurrentError::CurrentMissing(directory))
        }
    }
}

impl From<DiscoveryError> for CaptureCurrentError {
    fn from(source: DiscoveryError) -> Self {
        Self::Discovery(source)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use std::fs::{self, File};
    use std::io::{Seek, SeekFrom, Write};

    use tempfile::TempDir;
    use uuid::Uuid;

    use crate::discovery::save_directory_in;
    use crate::mutation_guard::MUTATION_GUARD_TEST;
    use crate::save_file::{SAVE_FILE_SIZE, validate_and_fingerprint};
    use crate::transaction::TRANSACTIONS_DIRECTORY_NAME;

    use super::*;

    fn create_save(path: &Path, marker: u8) {
        let mut file = File::create(path).unwrap();
        file.set_len(SAVE_FILE_SIZE).unwrap();
        file.seek(SeekFrom::Start(SAVE_FILE_SIZE - 1)).unwrap();
        file.write_all(&[marker]).unwrap();
        file.sync_all().unwrap();
    }

    #[test]
    fn manually_captures_current_as_a_stash_without_changing_current() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let documents = directory.path().join("Documents");
        let save_directory = save_directory_in(&documents);
        fs::create_dir_all(&save_directory).unwrap();
        let current = save_directory.join("Vwings.dat");
        create_save(&current, 1);
        let fingerprint = validate_and_fingerprint(&current).unwrap();

        let captured = capture_current_as_stash_in_documents(
            &repository,
            &documents,
            CaptureCurrentRequest {
                alias: Some("Before experimenting".into()),
                description: Some("Manual recovery point".into()),
            },
        )
        .unwrap();

        assert_eq!(StoredSaveKind::Stash, captured.metadata.kind);
        assert_eq!(StoredSaveOrigin::Current, captured.metadata.origin);
        assert_eq!(fingerprint, captured.metadata.fingerprint);
        assert_eq!(fingerprint, validate_and_fingerprint(&current).unwrap());
        assert_eq!(
            fingerprint,
            repository.verify(&captured.metadata.id).unwrap()
        );
    }

    #[test]
    fn captures_current_as_a_preset() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let documents = directory.path().join("Documents");
        let save_directory = save_directory_in(&documents);
        fs::create_dir_all(&save_directory).unwrap();
        let current = save_directory.join("Vwings.dat");
        create_save(&current, 1);

        let captured = capture_current_as_preset_in_documents(
            &repository,
            &documents,
            CaptureCurrentRequest {
                alias: Some("Practice start".into()),
                description: None,
            },
        )
        .unwrap();

        assert_eq!(StoredSaveKind::Preset, captured.metadata.kind);
        assert_eq!(StoredSaveOrigin::Current, captured.metadata.origin);
        assert_eq!(
            validate_and_fingerprint(&current).unwrap(),
            repository.verify(&captured.metadata.id).unwrap()
        );
    }

    #[test]
    fn generates_a_default_alias_when_none_is_provided() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let documents = directory.path().join("Documents");
        let save_directory = save_directory_in(&documents);
        fs::create_dir_all(&save_directory).unwrap();
        create_save(&save_directory.join("Vwings.dat"), 1);

        let captured = capture_current_as_stash_in_documents(
            &repository,
            &documents,
            CaptureCurrentRequest {
                alias: None,
                description: None,
            },
        )
        .unwrap();

        assert!(captured.metadata.alias.starts_with("Stash "));
    }

    #[test]
    fn reports_missing_current_states_and_ignores_backups() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let documents = directory.path().join("Documents");
        let save_directory = save_directory_in(&documents);

        let missing_directory = capture_current_as_stash_in_documents(
            &repository,
            &documents,
            CaptureCurrentRequest {
                alias: Some("Missing".into()),
                description: None,
            },
        );
        assert!(matches!(
            missing_directory,
            Err(CaptureCurrentError::SaveDirectoryMissing(_))
        ));

        fs::create_dir_all(&save_directory).unwrap();
        let missing_current = capture_current_as_stash_in_documents(
            &repository,
            &documents,
            CaptureCurrentRequest {
                alias: Some("Missing".into()),
                description: None,
            },
        );
        assert!(matches!(
            missing_current,
            Err(CaptureCurrentError::CurrentMissing(_))
        ));

        create_save(&save_directory.join("first.dat"), 1);
        create_save(&save_directory.join("second.dat"), 2);
        let backups_only = capture_current_as_stash_in_documents(
            &repository,
            &documents,
            CaptureCurrentRequest {
                alias: Some("Missing".into()),
                description: None,
            },
        );
        assert!(matches!(
            backups_only,
            Err(CaptureCurrentError::CurrentMissing(_))
        ));
        assert!(repository.list().unwrap().is_empty());
    }

    #[test]
    fn unfinished_transaction_blocks_manual_capture() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let documents = directory.path().join("Documents");
        let save_directory = save_directory_in(&documents);
        fs::create_dir_all(&save_directory).unwrap();
        create_save(&save_directory.join("Vwings.dat"), 1);
        let transactions = repository.root().join(TRANSACTIONS_DIRECTORY_NAME);
        fs::create_dir_all(&transactions).unwrap();
        let journal_path = transactions.join(format!("{}.json", Uuid::new_v4()));
        fs::write(&journal_path, b"unfinished").unwrap();

        let result = capture_current_as_stash_in_documents(
            &repository,
            &documents,
            CaptureCurrentRequest {
                alias: Some("Blocked".into()),
                description: None,
            },
        );

        assert!(matches!(
            result,
            Err(CaptureCurrentError::RecoveryRequired(paths)) if paths == vec![journal_path]
        ));
        assert!(repository.list().unwrap().is_empty());
    }
}
