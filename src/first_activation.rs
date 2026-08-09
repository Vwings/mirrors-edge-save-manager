use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::discovery::{self, CurrentSaveDiscovery, DiscoveryError};
use crate::game_process::GameProcessError;
use crate::known_folders;
use crate::mutation_guard::{MutationGuard, MutationGuardError};
use crate::recovery::{RecoveryError, unfinished_journals};
use crate::save_file::{SaveFileError, SaveFingerprint, validate_and_fingerprint};
use crate::staging::{StagingError, stage_stored_save};
use crate::storage::StoredSaveRepository;
use crate::transaction::{ActivationJournal, JournalStore};
use crate::windows_file;

pub struct ActivateCurrentRequest<'a> {
    pub stored_save_id: &'a str,
    pub confirmed_filename: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivateCurrentResult {
    pub current_path: PathBuf,
    pub fingerprint: SaveFingerprint,
}

#[derive(Debug)]
pub enum FirstActivationError {
    AccountName(io::Error),
    InvalidUnicode,
    InvalidFilenameStem(String),
    FilenameNotConfirmed {
        expected: String,
        actual: String,
    },
    MutationGuard(MutationGuardError),
    Recovery(RecoveryError),
    RecoveryRequired(Vec<PathBuf>),
    Discovery(Box<DiscoveryError>),
    SaveDirectoryMissing(PathBuf),
    CurrentAlreadyExists(PathBuf),
    GameProcess(GameProcessError),
    GameRunning,
    Staging(StagingError),
    Journal {
        path: PathBuf,
        source: io::Error,
    },
    Publish {
        path: PathBuf,
        source: io::Error,
    },
    SaveFile(SaveFileError),
    FingerprintMismatch {
        expected: SaveFingerprint,
        actual: SaveFingerprint,
    },
    Cleanup {
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for FirstActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountName(source) => {
                write!(
                    formatter,
                    "failed to read the Windows account name: {source}"
                )
            }
            Self::InvalidUnicode => {
                formatter.write_str("Windows returned an invalid Unicode account name")
            }
            Self::InvalidFilenameStem(name) => write!(
                formatter,
                "Windows account name is not a safe save filename stem: {name:?}"
            ),
            Self::FilenameNotConfirmed { expected, actual } => write!(
                formatter,
                "first activation filename was not confirmed: expected {expected:?}, got {actual:?}"
            ),
            Self::MutationGuard(source) => write!(formatter, "activation is blocked: {source}"),
            Self::Recovery(source) => write!(formatter, "transaction scan failed: {source}"),
            Self::RecoveryRequired(paths) => write!(
                formatter,
                "unfinished transaction recovery is required before activation: {paths:?}"
            ),
            Self::Discovery(source) => write!(formatter, "Current discovery failed: {source}"),
            Self::SaveDirectoryMissing(path) => {
                write!(
                    formatter,
                    "save directory does not exist: {}",
                    path.display()
                )
            }
            Self::CurrentAlreadyExists(path) => {
                write!(formatter, "Current already exists: {}", path.display())
            }
            Self::GameProcess(source) => write!(formatter, "game process check failed: {source}"),
            Self::GameRunning => formatter.write_str("Mirror's Edge started before activation"),
            Self::Staging(source) => write!(formatter, "activation staging failed: {source}"),
            Self::Journal { path, source } => write!(
                formatter,
                "failed to publish activation journal {}: {source}",
                path.display()
            ),
            Self::Publish { path, source } => write!(
                formatter,
                "failed to publish Current at {}: {source}",
                path.display()
            ),
            Self::SaveFile(source) => write!(formatter, "Current validation failed: {source}"),
            Self::FingerprintMismatch { expected, actual } => write!(
                formatter,
                "activated Current fingerprint is {actual:?}, expected {expected:?}"
            ),
            Self::Cleanup { path, source } => {
                write!(formatter, "failed to clean up {}: {source}", path.display())
            }
        }
    }
}

impl Error for FirstActivationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AccountName(source) => Some(source),
            Self::MutationGuard(source) => Some(source),
            Self::Recovery(source) => Some(source),
            Self::Discovery(source) => Some(source),
            Self::GameProcess(source) => Some(source),
            Self::Staging(source) => Some(source),
            Self::Journal { source, .. }
            | Self::Publish { source, .. }
            | Self::Cleanup { source, .. } => Some(source),
            Self::SaveFile(source) => Some(source),
            Self::InvalidUnicode
            | Self::InvalidFilenameStem(_)
            | Self::FilenameNotConfirmed { .. }
            | Self::RecoveryRequired(_)
            | Self::SaveDirectoryMissing(_)
            | Self::CurrentAlreadyExists(_)
            | Self::GameRunning
            | Self::FingerprintMismatch { .. } => None,
        }
    }
}

pub fn activate_current(
    repository: &StoredSaveRepository,
    request: ActivateCurrentRequest<'_>,
) -> Result<ActivateCurrentResult, FirstActivationError> {
    let documents = known_folders::documents().map_err(|source| {
        FirstActivationError::Discovery(Box::new(DiscoveryError::KnownFolder(source)))
    })?;
    activate_current_in_documents(repository, &documents, request)
}

pub fn activate_current_in_documents(
    repository: &StoredSaveRepository,
    documents_directory: &Path,
    request: ActivateCurrentRequest<'_>,
) -> Result<ActivateCurrentResult, FirstActivationError> {
    let _guard = MutationGuard::acquire().map_err(FirstActivationError::MutationGuard)?;
    let unfinished =
        unfinished_journals(repository.root()).map_err(FirstActivationError::Recovery)?;
    if !unfinished.is_empty() {
        return Err(FirstActivationError::RecoveryRequired(unfinished));
    }
    require_missing_current(discovery::discover_current_in_documents(
        documents_directory,
    )?)?;
    let current_path = discovery::current_path_in_documents(documents_directory)?;
    let expected_filename = current_path
        .file_name()
        .expect("the account-named Current path has a filename")
        .to_string_lossy()
        .into_owned();
    if request.confirmed_filename != expected_filename {
        return Err(FirstActivationError::FilenameNotConfirmed {
            expected: expected_filename,
            actual: request.confirmed_filename,
        });
    }

    let transaction_id = Uuid::new_v4().to_string();
    let staged = stage_stored_save(
        repository,
        request.stored_save_id,
        &current_path,
        &transaction_id,
    )?;
    let journal = ActivationJournal::new(
        transaction_id.clone(),
        request.stored_save_id.into(),
        current_path.clone(),
        staged.path.clone(),
        staged.fingerprint,
    )
    .map_err(|source| journal_error(repository, &transaction_id, source))?;
    let mut journal_store = JournalStore::new(repository.root(), &transaction_id);
    if let Err(source) = journal_store.publish(&journal) {
        remove_file(&staged.path)?;
        return Err(journal_error(repository, &transaction_id, source));
    }

    match is_game_running_before_activation() {
        Ok(false) => {}
        Ok(true) => {
            abort_activation(staged.path, journal_store)?;
            return Err(FirstActivationError::GameRunning);
        }
        Err(source) => {
            abort_activation(staged.path, journal_store)?;
            return Err(FirstActivationError::GameProcess(source));
        }
    }
    match discovery::discover_current_in_documents(documents_directory)? {
        CurrentSaveDiscovery::CurrentMissing { .. } => {}
        CurrentSaveDiscovery::SaveDirectoryMissing { directory } => {
            abort_activation(staged.path, journal_store)?;
            return Err(FirstActivationError::SaveDirectoryMissing(directory));
        }
        CurrentSaveDiscovery::CurrentFound(current) => {
            let path = current.path().to_path_buf();
            abort_activation(staged.path, journal_store)?;
            return Err(FirstActivationError::CurrentAlreadyExists(path));
        }
    }

    windows_file::atomic_move(&staged.path, &current_path, false).map_err(|source| {
        FirstActivationError::Publish {
            path: current_path.clone(),
            source,
        }
    })?;
    let fingerprint = validate_and_fingerprint(&current_path)?;
    if fingerprint != staged.fingerprint {
        return Err(FirstActivationError::FingerprintMismatch {
            expected: staged.fingerprint,
            actual: fingerprint,
        });
    }
    let journal_path = journal_store.path().to_path_buf();
    journal_store
        .remove()
        .map_err(|source| FirstActivationError::Cleanup {
            path: journal_path,
            source,
        })?;

    Ok(ActivateCurrentResult {
        current_path,
        fingerprint,
    })
}

fn require_missing_current(discovery: CurrentSaveDiscovery) -> Result<(), FirstActivationError> {
    match discovery {
        CurrentSaveDiscovery::CurrentMissing { .. } => Ok(()),
        CurrentSaveDiscovery::SaveDirectoryMissing { directory } => {
            Err(FirstActivationError::SaveDirectoryMissing(directory))
        }
        CurrentSaveDiscovery::CurrentFound(current) => Err(
            FirstActivationError::CurrentAlreadyExists(current.path().to_path_buf()),
        ),
    }
}

fn abort_activation(
    staged_path: PathBuf,
    journal_store: JournalStore,
) -> Result<(), FirstActivationError> {
    remove_file(&staged_path)?;
    let journal_path = journal_store.path().to_path_buf();
    journal_store
        .remove()
        .map_err(|source| FirstActivationError::Cleanup {
            path: journal_path,
            source,
        })
}

fn remove_file(path: &Path) -> Result<(), FirstActivationError> {
    fs::remove_file(path).map_err(|source| FirstActivationError::Cleanup {
        path: path.to_path_buf(),
        source,
    })
}

fn journal_error(
    repository: &StoredSaveRepository,
    transaction_id: &str,
    source: io::Error,
) -> FirstActivationError {
    FirstActivationError::Journal {
        path: repository
            .root()
            .join("transactions")
            .join(format!("{transaction_id}.json")),
        source,
    }
}

#[cfg(test)]
fn is_game_running_before_activation() -> Result<bool, GameProcessError> {
    Ok(false)
}

#[cfg(not(test))]
fn is_game_running_before_activation() -> Result<bool, GameProcessError> {
    crate::game_process::is_game_running()
}

impl From<DiscoveryError> for FirstActivationError {
    fn from(source: DiscoveryError) -> Self {
        Self::Discovery(Box::new(source))
    }
}

impl From<StagingError> for FirstActivationError {
    fn from(source: StagingError) -> Self {
        Self::Staging(source)
    }
}

impl From<SaveFileError> for FirstActivationError {
    fn from(source: SaveFileError) -> Self {
        Self::SaveFile(source)
    }
}

pub fn suggested_current_filename() -> Result<String, FirstActivationError> {
    let username = windows_account_name()?;
    validate_filename_stem(&username)?;
    Ok(format!("{username}.dat"))
}

pub fn confirm_current_filename(filename: &OsStr) -> Result<String, FirstActivationError> {
    let expected = suggested_current_filename()?;
    let actual = filename.to_string_lossy().into_owned();
    if actual == expected {
        Ok(expected)
    } else {
        Err(FirstActivationError::FilenameNotConfirmed { expected, actual })
    }
}

#[cfg(windows)]
fn windows_account_name() -> Result<String, FirstActivationError> {
    use windows_sys::Win32::System::WindowsProgramming::GetUserNameW;

    const MAX_ACCOUNT_NAME_LENGTH: usize = 256;
    let mut buffer = [0u16; MAX_ACCOUNT_NAME_LENGTH + 1];
    let mut length = buffer.len() as u32;
    if unsafe { GetUserNameW(buffer.as_mut_ptr(), &mut length) } == 0 {
        return Err(FirstActivationError::AccountName(io::Error::last_os_error()));
    }
    let length = usize::try_from(length).expect("account name buffer length fits usize");
    let without_null = length.saturating_sub(1);
    String::from_utf16(&buffer[..without_null]).map_err(|_| FirstActivationError::InvalidUnicode)
}

#[cfg(not(windows))]
fn windows_account_name() -> Result<String, FirstActivationError> {
    Err(FirstActivationError::AccountName(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows account names are only available on Windows",
    )))
}

fn validate_filename_stem(stem: &str) -> Result<(), FirstActivationError> {
    let invalid_character = |character: char| {
        character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
    };
    let reserved = ["CON", "PRN", "AUX", "NUL"];
    let reserved_numbered = ["COM", "LPT"];
    let upper = stem.to_ascii_uppercase();
    let device_name = upper.split('.').next().unwrap_or_default();
    let invalid = stem.is_empty()
        || stem == "."
        || stem == ".."
        || stem.ends_with([' ', '.'])
        || stem.encode_utf16().count() + ".dat".len() > 255
        || stem.chars().any(invalid_character)
        || reserved.contains(&device_name)
        || reserved_numbered.iter().any(|prefix| {
            device_name.strip_prefix(prefix).is_some_and(|number| {
                matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
        });
    if invalid {
        Err(FirstActivationError::InvalidFilenameStem(stem.into()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use std::fs::File;
    #[cfg(windows)]
    use std::io::{Seek, SeekFrom, Write};

    #[cfg(windows)]
    use tempfile::TempDir;

    #[cfg(windows)]
    use crate::discovery::save_directory_in;
    #[cfg(windows)]
    use crate::mutation_guard::MUTATION_GUARD_TEST;
    #[cfg(windows)]
    use crate::save_file::SAVE_FILE_SIZE;
    #[cfg(windows)]
    use crate::stored_save::{StoredSaveKind, StoredSaveOrigin};

    use super::*;

    #[cfg(windows)]
    fn create_save(path: &Path, marker: u8) {
        let mut file = File::create(path).unwrap();
        file.set_len(SAVE_FILE_SIZE).unwrap();
        file.seek(SeekFrom::Start(SAVE_FILE_SIZE - 1)).unwrap();
        file.write_all(&[marker]).unwrap();
        file.sync_all().unwrap();
    }

    #[test]
    fn rejects_unsafe_or_reserved_filename_stems() {
        for stem in [
            "", ".", "..", "bad/name", "bad.", "NUL", "CON.user", "COM1", "lpt9",
        ] {
            assert!(validate_filename_stem(stem).is_err(), "accepted {stem:?}");
        }
        assert!(validate_filename_stem("Vwings").is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn suggests_and_confirms_the_windows_account_filename() {
        let suggested = suggested_current_filename().unwrap();

        assert!(suggested.ends_with(".dat"));
        assert_eq!(
            suggested,
            confirm_current_filename(OsStr::new(&suggested)).unwrap()
        );
        assert!(confirm_current_filename(OsStr::new("someone-else.dat")).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn activates_account_named_current_without_touching_backups() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let source = directory.path().join("source.dat");
        create_save(&source, 2);
        let stored = repository
            .capture(
                &source,
                StoredSaveKind::Preset,
                "Practice".into(),
                None,
                StoredSaveOrigin::Imported,
            )
            .unwrap();
        let documents = directory.path().join("Documents");
        let save_directory = save_directory_in(&documents);
        fs::create_dir_all(&save_directory).unwrap();
        let backup = save_directory.join("old-run.dat");
        fs::write(&backup, b"backup").unwrap();

        let result = activate_current_in_documents(
            &repository,
            &documents,
            ActivateCurrentRequest {
                stored_save_id: &stored.metadata.id,
                confirmed_filename: "Vwings.dat".into(),
            },
        )
        .unwrap();

        assert_eq!(save_directory.join("Vwings.dat"), result.current_path);
        assert_eq!(stored.metadata.fingerprint, result.fingerprint);
        assert_eq!(b"backup", fs::read(backup).unwrap().as_slice());
        assert!(unfinished_journals(repository.root()).unwrap().is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn refuses_unconfirmed_or_existing_current_without_overwriting() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let source = directory.path().join("source.dat");
        create_save(&source, 2);
        let stored = repository
            .capture(
                &source,
                StoredSaveKind::Preset,
                "Practice".into(),
                None,
                StoredSaveOrigin::Imported,
            )
            .unwrap();
        let documents = directory.path().join("Documents");
        let save_directory = save_directory_in(&documents);
        fs::create_dir_all(&save_directory).unwrap();

        let unconfirmed = activate_current_in_documents(
            &repository,
            &documents,
            ActivateCurrentRequest {
                stored_save_id: &stored.metadata.id,
                confirmed_filename: "other.dat".into(),
            },
        );
        assert!(matches!(
            unconfirmed,
            Err(FirstActivationError::FilenameNotConfirmed { .. })
        ));

        let current = save_directory.join("Vwings.dat");
        create_save(&current, 1);
        let original = validate_and_fingerprint(&current).unwrap();
        let existing = activate_current_in_documents(
            &repository,
            &documents,
            ActivateCurrentRequest {
                stored_save_id: &stored.metadata.id,
                confirmed_filename: "Vwings.dat".into(),
            },
        );
        assert!(matches!(
            existing,
            Err(FirstActivationError::CurrentAlreadyExists(_))
        ));
        assert_eq!(original, validate_and_fingerprint(&current).unwrap());
    }
}
