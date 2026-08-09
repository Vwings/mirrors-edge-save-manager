use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::discovery;
use crate::known_folders::{self, KnownFolderError};
use crate::mutation_guard::{MutationGuard, MutationGuardError};
use crate::save_file::{SaveFingerprint, validate_and_fingerprint};
use crate::storage::StoredSaveRepository;
use crate::transaction::{
    ActivationJournal, ActivationPhase, ApplyJournal, ApplyPhase, JOURNAL_SCHEMA_VERSION,
    TRANSACTIONS_DIRECTORY_NAME,
};
use crate::windows_file;

const ARTIFACT_PREFIX: &str = ".mirrors-edge-save-manager-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    AbortedReplacement,
    DiscardedLostStaging,
    FinishedReplacement,
    FinishedVerifiedCleanup,
    RemovedDuplicateArtifacts,
    RestoredMissingCurrent,
    FinishedRollback,
    FinishedActivation,
    DiscardedLostActivationStaging,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredTransaction {
    pub transaction_id: String,
    pub action: RecoveryAction,
}

#[derive(Debug)]
pub enum RecoveryError {
    KnownFolder(KnownFolderError),
    MutationGuard(MutationGuardError),
    Scan {
        path: PathBuf,
        source: io::Error,
    },
    InvalidJournal {
        path: PathBuf,
        reason: String,
    },
    Blocked {
        journal_path: PathBuf,
        reason: String,
    },
    FileOperation {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for RecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KnownFolder(source) => {
                write!(formatter, "save location discovery failed: {source}")
            }
            Self::MutationGuard(source) => write!(formatter, "recovery is blocked: {source}"),
            Self::Scan { path, source } => {
                write!(formatter, "failed to scan {}: {source}", path.display())
            }
            Self::InvalidJournal { path, reason } => {
                write!(
                    formatter,
                    "invalid transaction journal {}: {reason}",
                    path.display()
                )
            }
            Self::Blocked {
                journal_path,
                reason,
            } => write!(
                formatter,
                "transaction recovery is blocked for {}: {reason}",
                journal_path.display()
            ),
            Self::FileOperation {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} {} during recovery: {source}",
                path.display()
            ),
        }
    }
}

impl Error for RecoveryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::KnownFolder(source) => Some(source),
            Self::MutationGuard(source) => Some(source),
            Self::Scan { source, .. } | Self::FileOperation { source, .. } => Some(source),
            Self::InvalidJournal { .. } | Self::Blocked { .. } => None,
        }
    }
}

pub fn recover_unfinished_transactions(
    repository: &StoredSaveRepository,
) -> Result<Vec<RecoveredTransaction>, RecoveryError> {
    let documents = known_folders::documents().map_err(RecoveryError::KnownFolder)?;
    recover_unfinished_transactions_in_documents(repository, &documents)
}

pub fn recover_unfinished_transactions_in_documents(
    repository: &StoredSaveRepository,
    documents_directory: &Path,
) -> Result<Vec<RecoveredTransaction>, RecoveryError> {
    let _guard = MutationGuard::acquire().map_err(RecoveryError::MutationGuard)?;
    recover_unfinished_transactions_guarded(repository, documents_directory)
}

pub(crate) fn unfinished_journals(root: &Path) -> Result<Vec<PathBuf>, RecoveryError> {
    let directory = root.join(TRANSACTIONS_DIRECTORY_NAME);
    if !directory
        .try_exists()
        .map_err(|source| RecoveryError::Scan {
            path: directory.clone(),
            source,
        })?
    {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&directory).map_err(|source| RecoveryError::Scan {
        path: directory.clone(),
        source,
    })?;
    let mut journals = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| RecoveryError::Scan {
            path: directory.clone(),
            source,
        })?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            journals.push(path);
        }
    }
    journals.sort();
    Ok(journals)
}

fn recover_unfinished_transactions_guarded(
    repository: &StoredSaveRepository,
    documents_directory: &Path,
) -> Result<Vec<RecoveredTransaction>, RecoveryError> {
    let journals = unfinished_journals(repository.root())?;
    let mut recovered = Vec::with_capacity(journals.len());
    for journal_path in journals {
        recovered.push(recover_one(&journal_path, documents_directory)?);
    }
    Ok(recovered)
}

fn recover_one(
    journal_path: &Path,
    documents_directory: &Path,
) -> Result<RecoveredTransaction, RecoveryError> {
    let bytes = fs::read(journal_path).map_err(|source| RecoveryError::FileOperation {
        operation: "read",
        path: journal_path.to_path_buf(),
        source,
    })?;
    let envelope: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|source| RecoveryError::InvalidJournal {
            path: journal_path.to_path_buf(),
            reason: source.to_string(),
        })?;
    match envelope.get("operation").and_then(|value| value.as_str()) {
        Some("apply") => {
            let journal: ApplyJournal =
                serde_json::from_slice(&bytes).map_err(|source| RecoveryError::InvalidJournal {
                    path: journal_path.to_path_buf(),
                    reason: source.to_string(),
                })?;
            recover_apply(journal_path, documents_directory, journal)
        }
        Some("activate") => {
            let journal: ActivationJournal =
                serde_json::from_slice(&bytes).map_err(|source| RecoveryError::InvalidJournal {
                    path: journal_path.to_path_buf(),
                    reason: source.to_string(),
                })?;
            recover_activation(journal_path, documents_directory, journal)
        }
        Some(operation) => Err(RecoveryError::InvalidJournal {
            path: journal_path.to_path_buf(),
            reason: format!("unsupported operation {operation}"),
        }),
        None => Err(RecoveryError::InvalidJournal {
            path: journal_path.to_path_buf(),
            reason: "missing operation".into(),
        }),
    }
}

fn recover_apply(
    journal_path: &Path,
    documents_directory: &Path,
    journal: ApplyJournal,
) -> Result<RecoveredTransaction, RecoveryError> {
    validate_apply_journal(journal_path, &journal, documents_directory)?;
    let original = SaveFingerprint::try_from(&journal.original_fingerprint).map_err(|source| {
        RecoveryError::InvalidJournal {
            path: journal_path.to_path_buf(),
            reason: source.to_string(),
        }
    })?;
    let replacement =
        SaveFingerprint::try_from(&journal.replacement_fingerprint).map_err(|source| {
            RecoveryError::InvalidJournal {
                path: journal_path.to_path_buf(),
                reason: source.to_string(),
            }
        })?;
    let current = inspect(&journal.current_path);
    let staging = inspect(&journal.replacement_path);
    let rollback = inspect(&journal.rollback_path);
    let failed = inspect(&journal.failed_replacement_path);

    let action = match (&current, &staging, &rollback, &failed) {
        (
            Observed::Fingerprint(value),
            Observed::Fingerprint(staged),
            Observed::Missing,
            Observed::Missing,
        ) if *value == original && *staged == replacement => {
            remove(&journal.replacement_path)?;
            remove(journal_path)?;
            RecoveryAction::AbortedReplacement
        }
        (Observed::Fingerprint(value), Observed::Missing, Observed::Missing, Observed::Missing)
            if *value == original =>
        {
            remove(journal_path)?;
            RecoveryAction::DiscardedLostStaging
        }
        (
            Observed::Fingerprint(value),
            Observed::Missing,
            Observed::Fingerprint(old),
            Observed::Missing,
        ) if *value == replacement && *old == original => {
            validate_expected(&journal.current_path, replacement, journal_path)?;
            remove(&journal.rollback_path)?;
            remove(journal_path)?;
            RecoveryAction::FinishedReplacement
        }
        (Observed::Fingerprint(value), Observed::Missing, Observed::Missing, Observed::Missing)
            if *value == replacement && journal.phase == ApplyPhase::Verified =>
        {
            validate_expected(&journal.current_path, replacement, journal_path)?;
            remove(journal_path)?;
            RecoveryAction::FinishedVerifiedCleanup
        }
        (Observed::Fingerprint(value), staged, Observed::Fingerprint(old), Observed::Missing)
            if *value == original && is_missing_or(staged, replacement) && *old == original =>
        {
            if matches!(staged, Observed::Fingerprint(_)) {
                remove(&journal.replacement_path)?;
            }
            remove(&journal.rollback_path)?;
            remove(journal_path)?;
            RecoveryAction::RemovedDuplicateArtifacts
        }
        (Observed::Missing, staged, Observed::Fingerprint(old), Observed::Missing)
            if is_missing_or(staged, replacement) && *old == original =>
        {
            windows_file::atomic_move(&journal.rollback_path, &journal.current_path, false)
                .map_err(|source| RecoveryError::FileOperation {
                    operation: "restore",
                    path: journal.current_path.clone(),
                    source,
                })?;
            validate_expected(&journal.current_path, original, journal_path)?;
            if matches!(staged, Observed::Fingerprint(_)) {
                remove(&journal.replacement_path)?;
            }
            remove(journal_path)?;
            RecoveryAction::RestoredMissingCurrent
        }
        (
            Observed::Fingerprint(value),
            Observed::Missing,
            Observed::Missing,
            Observed::Fingerprint(failed_value),
        ) if *value == original
            && *failed_value == replacement
            && journal.phase == ApplyPhase::RollingBack =>
        {
            validate_expected(&journal.current_path, original, journal_path)?;
            remove(&journal.failed_replacement_path)?;
            remove(journal_path)?;
            RecoveryAction::FinishedRollback
        }
        _ => {
            return Err(RecoveryError::Blocked {
                journal_path: journal_path.to_path_buf(),
                reason: format!(
                    "artifact fingerprints do not match a safe recovery state: current={current}, staging={staging}, rollback={rollback}, failed={failed}"
                ),
            });
        }
    };

    Ok(RecoveredTransaction {
        transaction_id: journal.transaction_id,
        action,
    })
}

fn recover_activation(
    journal_path: &Path,
    documents_directory: &Path,
    journal: ActivationJournal,
) -> Result<RecoveredTransaction, RecoveryError> {
    validate_activation_journal(journal_path, &journal, documents_directory)?;
    let replacement =
        SaveFingerprint::try_from(&journal.replacement_fingerprint).map_err(|source| {
            RecoveryError::InvalidJournal {
                path: journal_path.to_path_buf(),
                reason: source.to_string(),
            }
        })?;
    let current = inspect(&journal.current_path);
    let staging = inspect(&journal.staging_path);

    let action = match (&current, &staging) {
        (Observed::Missing, Observed::Fingerprint(staged)) if *staged == replacement => {
            windows_file::atomic_move(&journal.staging_path, &journal.current_path, false)
                .map_err(|source| RecoveryError::FileOperation {
                    operation: "finish activation",
                    path: journal.current_path.clone(),
                    source,
                })?;
            validate_expected(&journal.current_path, replacement, journal_path)?;
            remove(journal_path)?;
            RecoveryAction::FinishedActivation
        }
        (Observed::Missing, Observed::Missing) => {
            remove(journal_path)?;
            RecoveryAction::DiscardedLostActivationStaging
        }
        (Observed::Fingerprint(value), Observed::Missing) if *value == replacement => {
            validate_expected(&journal.current_path, replacement, journal_path)?;
            remove(journal_path)?;
            RecoveryAction::FinishedActivation
        }
        _ => {
            return Err(RecoveryError::Blocked {
                journal_path: journal_path.to_path_buf(),
                reason: format!(
                    "activation artifacts do not match a safe recovery state: current={current}, staging={staging}"
                ),
            });
        }
    };

    Ok(RecoveredTransaction {
        transaction_id: journal.transaction_id,
        action,
    })
}

fn validate_activation_journal(
    path: &Path,
    journal: &ActivationJournal,
    documents_directory: &Path,
) -> Result<(), RecoveryError> {
    let invalid = |reason: String| RecoveryError::InvalidJournal {
        path: path.to_path_buf(),
        reason,
    };
    if journal.schema_version != JOURNAL_SCHEMA_VERSION {
        return Err(invalid(format!(
            "unsupported schema version {}",
            journal.schema_version
        )));
    }
    if journal.operation != "activate" {
        return Err(invalid(format!(
            "unsupported operation {}",
            journal.operation
        )));
    }
    if journal.phase != ActivationPhase::Prepared {
        return Err(invalid("unsupported activation phase".into()));
    }
    let transaction_id = Uuid::parse_str(&journal.transaction_id)
        .map_err(|source| invalid(format!("invalid transaction ID: {source}")))?;
    if transaction_id.to_string() != journal.transaction_id {
        return Err(invalid("transaction ID is not canonical".into()));
    }
    if path.file_name().and_then(|name| name.to_str())
        != Some(&format!("{}.json", journal.transaction_id))
    {
        return Err(invalid(
            "journal filename does not match transaction ID".into(),
        ));
    }
    Uuid::parse_str(&journal.stored_save_id)
        .map_err(|source| invalid(format!("invalid StoredSave ID: {source}")))?;
    let expected_current = discovery::current_path_in_documents(documents_directory)
        .map_err(|source| invalid(source.to_string()))?;
    if journal.current_path != expected_current {
        return Err(invalid(
            "Current path is not the native account-named save path".into(),
        ));
    }
    let expected_staging = expected_current
        .parent()
        .expect("the native Current path has a parent")
        .join(format!(
            "{ARTIFACT_PREFIX}{}.replacement.dat",
            journal.transaction_id
        ));
    if journal.staging_path != expected_staging {
        return Err(invalid(
            "staging path does not match the derived transaction path".into(),
        ));
    }
    Ok(())
}

fn validate_apply_journal(
    path: &Path,
    journal: &ApplyJournal,
    documents_directory: &Path,
) -> Result<(), RecoveryError> {
    let invalid = |reason: String| RecoveryError::InvalidJournal {
        path: path.to_path_buf(),
        reason,
    };
    if journal.schema_version != JOURNAL_SCHEMA_VERSION {
        return Err(invalid(format!(
            "unsupported schema version {}",
            journal.schema_version
        )));
    }
    if journal.operation != "apply" {
        return Err(invalid(format!(
            "unsupported operation {}",
            journal.operation
        )));
    }
    let transaction_id = Uuid::parse_str(&journal.transaction_id)
        .map_err(|source| invalid(format!("invalid transaction ID: {source}")))?;
    if transaction_id.to_string() != journal.transaction_id {
        return Err(invalid("transaction ID is not canonical".into()));
    }
    if path.file_name().and_then(|name| name.to_str())
        != Some(&format!("{}.json", journal.transaction_id))
    {
        return Err(invalid(
            "journal filename does not match transaction ID".into(),
        ));
    }
    Uuid::parse_str(&journal.stored_save_id)
        .map_err(|source| invalid(format!("invalid StoredSave ID: {source}")))?;
    Uuid::parse_str(&journal.automatic_stash_id)
        .map_err(|source| invalid(format!("invalid automatic Stash ID: {source}")))?;
    let expected_current = discovery::current_path_in_documents(documents_directory)
        .map_err(|source| invalid(source.to_string()))?;
    if journal.current_path != expected_current {
        return Err(invalid(
            "Current path is not the native account-named save path".into(),
        ));
    }
    let directory = expected_current
        .parent()
        .expect("the native Current path has a parent");
    for (actual, kind) in [
        (&journal.replacement_path, "replacement"),
        (&journal.rollback_path, "rollback"),
        (&journal.failed_replacement_path, "failed"),
    ] {
        let expected = directory.join(format!(
            "{ARTIFACT_PREFIX}{}.{kind}.dat",
            journal.transaction_id
        ));
        if actual != &expected {
            return Err(invalid(format!(
                "{kind} path does not match the derived transaction path"
            )));
        }
    }
    Ok(())
}

#[derive(Debug)]
enum Observed {
    Missing,
    Fingerprint(SaveFingerprint),
    Invalid(String),
}

impl fmt::Display for Observed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => formatter.write_str("missing"),
            Self::Fingerprint(fingerprint) => write!(formatter, "{fingerprint:?}"),
            Self::Invalid(reason) => write!(formatter, "invalid ({reason})"),
        }
    }
}

fn is_missing_or(observed: &Observed, expected: SaveFingerprint) -> bool {
    matches!(observed, Observed::Missing)
        || matches!(observed, Observed::Fingerprint(actual) if *actual == expected)
}

fn inspect(path: &Path) -> Observed {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Observed::Missing,
        Err(error) => return Observed::Invalid(error.to_string()),
    };
    if !metadata.file_type().is_file() {
        return Observed::Invalid("path is not a regular file".into());
    }
    match validate_and_fingerprint(path) {
        Ok(fingerprint) => Observed::Fingerprint(fingerprint),
        Err(error) => Observed::Invalid(error.to_string()),
    }
}

fn validate_expected(
    path: &Path,
    expected: SaveFingerprint,
    journal_path: &Path,
) -> Result<(), RecoveryError> {
    let actual = validate_and_fingerprint(path).map_err(|source| RecoveryError::Blocked {
        journal_path: journal_path.to_path_buf(),
        reason: format!("restored Current could not be verified: {source}"),
    })?;
    if actual != expected {
        return Err(RecoveryError::Blocked {
            journal_path: journal_path.to_path_buf(),
            reason: format!("restored Current fingerprint is {actual:?}, expected {expected:?}"),
        });
    }
    Ok(())
}

fn remove(path: &Path) -> Result<(), RecoveryError> {
    fs::remove_file(path).map_err(|source| RecoveryError::FileOperation {
        operation: "remove",
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(all(test, windows))]
mod tests {
    use std::fs::File;
    use std::io::{Seek, SeekFrom, Write};

    use tempfile::TempDir;

    use crate::mutation_guard::MUTATION_GUARD_TEST;
    use crate::save_file::SAVE_FILE_SIZE;
    use crate::transaction::JournalStore;

    use super::*;

    #[derive(Clone, Copy)]
    enum RecoveryCase {
        Prepared,
        LostStaging,
        Replaced,
        VerifiedCleanup,
        DuplicateArtifacts,
        MissingCurrent,
        RolledBack,
    }

    fn create_save(path: &Path, marker: u8) -> SaveFingerprint {
        let mut file = File::create(path).unwrap();
        file.set_len(SAVE_FILE_SIZE).unwrap();
        file.seek(SeekFrom::Start(SAVE_FILE_SIZE - 1)).unwrap();
        file.write_all(&[marker]).unwrap();
        file.sync_all().unwrap();
        validate_and_fingerprint(path).unwrap()
    }

    #[test]
    fn recovers_every_safe_startup_state() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let cases = [
            (RecoveryCase::Prepared, RecoveryAction::AbortedReplacement),
            (
                RecoveryCase::LostStaging,
                RecoveryAction::DiscardedLostStaging,
            ),
            (RecoveryCase::Replaced, RecoveryAction::FinishedReplacement),
            (
                RecoveryCase::VerifiedCleanup,
                RecoveryAction::FinishedVerifiedCleanup,
            ),
            (
                RecoveryCase::DuplicateArtifacts,
                RecoveryAction::RemovedDuplicateArtifacts,
            ),
            (
                RecoveryCase::MissingCurrent,
                RecoveryAction::RestoredMissingCurrent,
            ),
            (RecoveryCase::RolledBack, RecoveryAction::FinishedRollback),
        ];

        for (case, expected_action) in cases {
            let directory = TempDir::new().unwrap();
            let repository = StoredSaveRepository::new(directory.path().join("app-data"));
            let transaction_id = Uuid::new_v4().to_string();
            let documents = directory.path().join("Documents");
            let save_directory = discovery::save_directory_in(&documents);
            fs::create_dir_all(&save_directory).unwrap();
            let current = save_directory.join("Vwings.dat");
            let replacement_path =
                save_directory.join(format!("{ARTIFACT_PREFIX}{transaction_id}.replacement.dat"));
            let rollback_path =
                save_directory.join(format!("{ARTIFACT_PREFIX}{transaction_id}.rollback.dat"));
            let failed_path =
                save_directory.join(format!("{ARTIFACT_PREFIX}{transaction_id}.failed.dat"));
            let original_source = directory.path().join("original.dat");
            let replacement_source = directory.path().join("new.dat");
            let original = create_save(&original_source, 1);
            let replacement = create_save(&replacement_source, 2);
            let mut journal = ApplyJournal::new(
                transaction_id.clone(),
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                current.clone(),
                replacement_path.clone(),
                rollback_path.clone(),
                failed_path.clone(),
                original,
                replacement,
            )
            .unwrap();

            match case {
                RecoveryCase::Prepared => {
                    fs::copy(&original_source, &current).unwrap();
                    fs::copy(&replacement_source, &replacement_path).unwrap();
                }
                RecoveryCase::LostStaging => {
                    fs::copy(&original_source, &current).unwrap();
                }
                RecoveryCase::Replaced => {
                    fs::copy(&replacement_source, &current).unwrap();
                    fs::copy(&original_source, &rollback_path).unwrap();
                    journal.set_phase(ApplyPhase::Replaced).unwrap();
                }
                RecoveryCase::VerifiedCleanup => {
                    fs::copy(&replacement_source, &current).unwrap();
                    journal.set_phase(ApplyPhase::Verified).unwrap();
                }
                RecoveryCase::DuplicateArtifacts => {
                    fs::copy(&original_source, &current).unwrap();
                    fs::copy(&replacement_source, &replacement_path).unwrap();
                    fs::copy(&original_source, &rollback_path).unwrap();
                }
                RecoveryCase::MissingCurrent => {
                    fs::copy(&replacement_source, &replacement_path).unwrap();
                    fs::copy(&original_source, &rollback_path).unwrap();
                    journal.set_phase(ApplyPhase::Replacing).unwrap();
                }
                RecoveryCase::RolledBack => {
                    fs::copy(&original_source, &current).unwrap();
                    fs::copy(&replacement_source, &failed_path).unwrap();
                    journal.set_phase(ApplyPhase::RollingBack).unwrap();
                }
            }
            let mut store = JournalStore::new(repository.root(), &transaction_id);
            store.publish(&journal).unwrap();

            let recovered =
                recover_unfinished_transactions_in_documents(&repository, &documents).unwrap();

            assert_eq!(
                vec![RecoveredTransaction {
                    transaction_id,
                    action: expected_action,
                }],
                recovered
            );
            let expected_current = match case {
                RecoveryCase::Replaced | RecoveryCase::VerifiedCleanup => replacement,
                _ => original,
            };
            assert_eq!(
                expected_current,
                validate_and_fingerprint(&current).unwrap()
            );
            assert!(!replacement_path.exists());
            assert!(!rollback_path.exists());
            assert!(!failed_path.exists());
            assert!(unfinished_journals(repository.root()).unwrap().is_empty());
        }
    }

    #[test]
    fn recovers_every_safe_first_activation_state() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        for (has_staging, has_current, expected_action) in [
            (true, false, RecoveryAction::FinishedActivation),
            (false, true, RecoveryAction::FinishedActivation),
            (false, false, RecoveryAction::DiscardedLostActivationStaging),
        ] {
            let directory = TempDir::new().unwrap();
            let repository = StoredSaveRepository::new(directory.path().join("app-data"));
            let documents = directory.path().join("Documents");
            let save_directory = discovery::save_directory_in(&documents);
            fs::create_dir_all(&save_directory).unwrap();
            let transaction_id = Uuid::new_v4().to_string();
            let current = save_directory.join("Vwings.dat");
            let staging =
                save_directory.join(format!("{ARTIFACT_PREFIX}{transaction_id}.replacement.dat"));
            let source = directory.path().join("source.dat");
            let fingerprint = create_save(&source, 2);
            if has_staging {
                fs::copy(&source, &staging).unwrap();
            }
            if has_current {
                fs::copy(&source, &current).unwrap();
            }
            let journal = ActivationJournal::new(
                transaction_id.clone(),
                Uuid::new_v4().to_string(),
                current.clone(),
                staging.clone(),
                fingerprint,
            )
            .unwrap();
            let mut store = JournalStore::new(repository.root(), &transaction_id);
            store.publish(&journal).unwrap();

            let recovered =
                recover_unfinished_transactions_in_documents(&repository, &documents).unwrap();

            assert_eq!(expected_action, recovered[0].action);
            if has_staging || has_current {
                assert_eq!(fingerprint, validate_and_fingerprint(&current).unwrap());
            } else {
                assert!(!current.exists());
            }
            assert!(!staging.exists());
            assert!(unfinished_journals(repository.root()).unwrap().is_empty());
        }
    }

    #[test]
    fn preserves_artifacts_when_fingerprints_are_contradictory() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let transaction_id = Uuid::new_v4().to_string();
        let documents = directory.path().join("Documents");
        let save_directory = discovery::save_directory_in(&documents);
        fs::create_dir_all(&save_directory).unwrap();
        let current = save_directory.join("Vwings.dat");
        let replacement_path =
            save_directory.join(format!("{ARTIFACT_PREFIX}{transaction_id}.replacement.dat"));
        let rollback_path =
            save_directory.join(format!("{ARTIFACT_PREFIX}{transaction_id}.rollback.dat"));
        let failed_path =
            save_directory.join(format!("{ARTIFACT_PREFIX}{transaction_id}.failed.dat"));
        let original = create_save(&current, 1);
        let replacement_source = directory.path().join("new.dat");
        let replacement = create_save(&replacement_source, 2);
        create_save(&rollback_path, 3);
        let journal = ApplyJournal::new(
            transaction_id.clone(),
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            current.clone(),
            replacement_path,
            rollback_path.clone(),
            failed_path,
            original,
            replacement,
        )
        .unwrap();
        let mut store = JournalStore::new(repository.root(), &transaction_id);
        store.publish(&journal).unwrap();

        let result = recover_unfinished_transactions_in_documents(&repository, &documents);

        assert!(matches!(result, Err(RecoveryError::Blocked { .. })));
        assert!(rollback_path.exists());
        assert!(store.path().exists());
    }

    #[test]
    fn malformed_journal_blocks_recovery() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let documents = directory.path().join("Documents");
        let transactions = repository.root().join(TRANSACTIONS_DIRECTORY_NAME);
        fs::create_dir_all(&transactions).unwrap();
        let journal_path = transactions.join(format!("{}.json", Uuid::new_v4()));
        fs::write(&journal_path, b"{not valid json").unwrap();

        let result = recover_unfinished_transactions_in_documents(&repository, &documents);

        assert!(matches!(result, Err(RecoveryError::InvalidJournal { .. })));
        assert!(journal_path.exists());
    }
}
