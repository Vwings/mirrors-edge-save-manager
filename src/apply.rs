use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::alias::{AliasError, resolve_current_alias};
use crate::discovery::{self, CurrentSaveDiscovery, DiscoveryError};
#[cfg(not(test))]
use crate::game_process;
use crate::game_process::GameProcessError;
use crate::known_folders;
use crate::mutation_guard::{MutationGuard, MutationGuardError};
use crate::recovery::{RecoveryError, unfinished_journals};
use crate::save_file::{SaveFileError, SaveFingerprint, validate_and_fingerprint};
use crate::staging::{StagingError, stage_stored_save};
use crate::storage::{CaptureResult, StorageError, StoredSaveRepository};
use crate::stored_save::{StoredSaveKind, StoredSaveOrigin};
use crate::transaction::{ApplyJournal, ApplyPhase, JournalStore};
use crate::windows_file;

const ARTIFACT_PREFIX: &str = ".mirrors-edge-save-manager-";

pub struct ApplyRequest<'a> {
    pub stored_save_id: &'a str,
    pub automatic_stash_alias: Option<String>,
    pub automatic_stash_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyResult {
    pub automatic_stash: CaptureResult,
    pub applied_fingerprint: SaveFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactState {
    Missing,
    Fingerprint(SaveFingerprint),
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactSnapshot {
    pub current: ArtifactState,
    pub replacement: ArtifactState,
    pub rollback: ArtifactState,
    pub failed_replacement: ArtifactState,
}

#[derive(Debug)]
pub enum ApplyError {
    Alias(AliasError),
    MutationGuard(MutationGuardError),
    Recovery(RecoveryError),
    RecoveryRequired(Vec<PathBuf>),
    Discovery(DiscoveryError),
    SaveDirectoryMissing(PathBuf),
    CurrentMissing(PathBuf),
    CurrentPathChanged {
        expected: PathBuf,
        actual: PathBuf,
    },
    CurrentFingerprintChanged {
        expected: SaveFingerprint,
        actual: SaveFingerprint,
    },
    GameProcess(GameProcessError),
    GameRunning,
    SaveFile(SaveFileError),
    Storage(StorageError),
    Staging(StagingError),
    ArtifactAlreadyExists(PathBuf),
    Journal {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Replace {
        source: io::Error,
        artifacts: Box<ArtifactSnapshot>,
    },
    ReplacementVerificationRolledBack(String),
    RollbackFailed {
        verification: String,
        rollback: Box<ApplyError>,
    },
    UnexpectedArtifacts(Box<ArtifactSnapshot>),
    Cleanup {
        path: PathBuf,
        source: io::Error,
    },
}

pub fn apply(
    repository: &StoredSaveRepository,
    request: ApplyRequest<'_>,
) -> Result<ApplyResult, ApplyError> {
    let documents = known_folders::documents()
        .map_err(|source| ApplyError::Discovery(DiscoveryError::KnownFolder(source)))?;
    apply_in_documents(repository, &documents, request)
}

impl fmt::Display for ApplyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alias(source) => write!(formatter, "invalid automatic Stash alias: {source}"),
            Self::MutationGuard(source) => write!(formatter, "mutation is blocked: {source}"),
            Self::Recovery(source) => write!(formatter, "transaction scan failed: {source}"),
            Self::RecoveryRequired(paths) => write!(
                formatter,
                "unfinished transaction recovery is required before apply: {paths:?}"
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
            Self::CurrentPathChanged { expected, actual } => write!(
                formatter,
                "Current path changed from {} to {} before replacement",
                expected.display(),
                actual.display()
            ),
            Self::CurrentFingerprintChanged { expected, actual } => write!(
                formatter,
                "Current changed before replacement: expected {expected:?}, got {actual:?}"
            ),
            Self::GameProcess(source) => write!(formatter, "game process check failed: {source}"),
            Self::GameRunning => {
                formatter.write_str("Mirror's Edge started before Current replacement")
            }
            Self::SaveFile(source) => write!(formatter, "Current validation failed: {source}"),
            Self::Storage(source) => write!(formatter, "StoredSave operation failed: {source}"),
            Self::Staging(source) => write!(formatter, "replacement staging failed: {source}"),
            Self::ArtifactAlreadyExists(path) => {
                write!(
                    formatter,
                    "transaction artifact already exists: {}",
                    path.display()
                )
            }
            Self::Journal {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} transaction journal {}: {source}",
                path.display()
            ),
            Self::Replace { source, .. } => {
                write!(formatter, "Current replacement failed: {source}")
            }
            Self::ReplacementVerificationRolledBack(reason) => write!(
                formatter,
                "replacement verification failed and the original Current was restored: {reason}"
            ),
            Self::RollbackFailed {
                verification,
                rollback,
            } => write!(
                formatter,
                "replacement verification failed ({verification}) and rollback failed: {rollback}"
            ),
            Self::UnexpectedArtifacts(_) => {
                formatter.write_str("replacement produced an unexpected artifact state")
            }
            Self::Cleanup { path, source } => {
                write!(formatter, "failed to clean up {}: {source}", path.display())
            }
        }
    }
}

impl Error for ApplyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Alias(source) => Some(source),
            Self::MutationGuard(source) => Some(source),
            Self::Recovery(source) => Some(source),
            Self::Discovery(source) => Some(source),
            Self::GameProcess(source) => Some(source),
            Self::SaveFile(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::Staging(source) => Some(source),
            Self::Journal { source, .. }
            | Self::Replace { source, .. }
            | Self::Cleanup { source, .. } => Some(source),
            Self::RollbackFailed { rollback, .. } => Some(rollback),
            Self::RecoveryRequired(_)
            | Self::SaveDirectoryMissing(_)
            | Self::CurrentMissing(_)
            | Self::CurrentPathChanged { .. }
            | Self::CurrentFingerprintChanged { .. }
            | Self::GameRunning
            | Self::ArtifactAlreadyExists(_)
            | Self::ReplacementVerificationRolledBack(_)
            | Self::UnexpectedArtifacts(_) => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ApplyBoundary {
    PublishPrepared,
    PublishReplacing,
    ReplaceCurrent,
    PublishReplaced,
    VerifyReplacement,
    PublishVerified,
    CommitRollbackCleanup,
    CommitJournalCleanup,
    PublishRollingBack,
    ReplaceRollback,
    VerifyRollback,
    RollbackFailedCleanup,
    RollbackJournalCleanup,
}

trait ApplyOperations {
    fn publish(
        &self,
        store: &mut JournalStore,
        journal: &ApplyJournal,
        boundary: ApplyBoundary,
    ) -> io::Result<()>;
    fn replace(
        &self,
        current: &Path,
        replacement: &Path,
        rollback: &Path,
        boundary: ApplyBoundary,
    ) -> io::Result<()>;
    fn validate(
        &self,
        path: &Path,
        boundary: ApplyBoundary,
    ) -> Result<SaveFingerprint, SaveFileError>;
    fn remove(&self, path: &Path, boundary: ApplyBoundary) -> io::Result<()>;
}

struct SystemApplyOperations;

impl ApplyOperations for SystemApplyOperations {
    fn publish(
        &self,
        store: &mut JournalStore,
        journal: &ApplyJournal,
        _boundary: ApplyBoundary,
    ) -> io::Result<()> {
        store.publish(journal)
    }

    fn replace(
        &self,
        current: &Path,
        replacement: &Path,
        rollback: &Path,
        _boundary: ApplyBoundary,
    ) -> io::Result<()> {
        windows_file::replace_file(current, replacement, rollback)
    }

    fn validate(
        &self,
        path: &Path,
        _boundary: ApplyBoundary,
    ) -> Result<SaveFingerprint, SaveFileError> {
        validate_and_fingerprint(path)
    }

    fn remove(&self, path: &Path, _boundary: ApplyBoundary) -> io::Result<()> {
        fs::remove_file(path)
    }
}

pub fn apply_in_documents(
    repository: &StoredSaveRepository,
    documents_directory: &Path,
    request: ApplyRequest<'_>,
) -> Result<ApplyResult, ApplyError> {
    apply_in_documents_with_operations(
        repository,
        documents_directory,
        request,
        || {},
        &SystemApplyOperations,
    )
}

#[cfg(all(test, windows))]
fn apply_in_documents_with_before_recheck(
    repository: &StoredSaveRepository,
    documents_directory: &Path,
    request: ApplyRequest<'_>,
    before_recheck: impl FnOnce(),
) -> Result<ApplyResult, ApplyError> {
    apply_in_documents_with_operations(
        repository,
        documents_directory,
        request,
        before_recheck,
        &SystemApplyOperations,
    )
}

fn apply_in_documents_with_operations(
    repository: &StoredSaveRepository,
    documents_directory: &Path,
    request: ApplyRequest<'_>,
    before_recheck: impl FnOnce(),
    operations: &impl ApplyOperations,
) -> Result<ApplyResult, ApplyError> {
    let _guard = MutationGuard::acquire().map_err(ApplyError::MutationGuard)?;
    let unfinished = unfinished_journals(repository.root()).map_err(ApplyError::Recovery)?;
    if !unfinished.is_empty() {
        return Err(ApplyError::RecoveryRequired(unfinished));
    }
    let current = require_current(discovery::discover_current_in_documents(
        documents_directory,
    )?)?;
    let current_path = current.path().to_path_buf();
    let original_fingerprint = validate_and_fingerprint(&current_path)?;
    let automatic_stash_alias =
        resolve_current_alias(request.automatic_stash_alias, "Stash").map_err(ApplyError::Alias)?;
    let automatic_stash = repository.capture(
        &current_path,
        StoredSaveKind::Stash,
        automatic_stash_alias,
        request.automatic_stash_description,
        StoredSaveOrigin::Current,
    )?;
    let transaction_id = Uuid::new_v4().to_string();
    let rollback_path = artifact_path(&current_path, &transaction_id, "rollback")?;
    let failed_replacement_path = artifact_path(&current_path, &transaction_id, "failed")?;
    require_missing(&rollback_path)?;
    require_missing(&failed_replacement_path)?;
    let staged = stage_stored_save(
        repository,
        request.stored_save_id,
        &current_path,
        &transaction_id,
    )?;
    let mut journal = ApplyJournal::new(
        transaction_id.clone(),
        request.stored_save_id.into(),
        automatic_stash.metadata.id.clone(),
        current_path.clone(),
        staged.path.clone(),
        rollback_path.clone(),
        failed_replacement_path.clone(),
        original_fingerprint,
        staged.fingerprint,
    )
    .map_err(|source| journal_error("create", repository, &transaction_id, source))?;
    let mut journal_store = JournalStore::new(repository.root(), &transaction_id);
    if let Err(source) =
        operations.publish(&mut journal_store, &journal, ApplyBoundary::PublishPrepared)
    {
        remove_file(&staged.path)?;
        return Err(journal_error(
            "publish",
            repository,
            &transaction_id,
            source,
        ));
    }

    before_recheck();
    match is_game_running_before_replace() {
        Ok(true) => {
            abort_before_replace(staged.path, journal_store)?;
            return Err(ApplyError::GameRunning);
        }
        Ok(false) => {}
        Err(source) => {
            abort_before_replace(staged.path, journal_store)?;
            return Err(ApplyError::GameProcess(source));
        }
    }
    let rediscovered = match discovery::discover_current_in_documents(documents_directory) {
        Ok(discovery) => match require_current(discovery) {
            Ok(current) => current,
            Err(error) => {
                abort_before_replace(staged.path, journal_store)?;
                return Err(error);
            }
        },
        Err(source) => {
            abort_before_replace(staged.path, journal_store)?;
            return Err(ApplyError::Discovery(source));
        }
    };
    if rediscovered.path() != current_path {
        let actual = rediscovered.path().to_path_buf();
        abort_before_replace(staged.path, journal_store)?;
        return Err(ApplyError::CurrentPathChanged {
            expected: current_path,
            actual,
        });
    }
    let actual_fingerprint = match validate_and_fingerprint(&current_path) {
        Ok(fingerprint) => fingerprint,
        Err(source) => {
            abort_before_replace(staged.path, journal_store)?;
            return Err(ApplyError::SaveFile(source));
        }
    };
    if actual_fingerprint != original_fingerprint {
        abort_before_replace(staged.path, journal_store)?;
        return Err(ApplyError::CurrentFingerprintChanged {
            expected: original_fingerprint,
            actual: actual_fingerprint,
        });
    }

    journal
        .set_phase(ApplyPhase::Replacing)
        .map_err(|source| journal_error("update", repository, &transaction_id, source))?;
    operations
        .publish(
            &mut journal_store,
            &journal,
            ApplyBoundary::PublishReplacing,
        )
        .map_err(|source| journal_error("publish", repository, &transaction_id, source))?;
    if let Err(source) = operations.replace(
        &current_path,
        &staged.path,
        &rollback_path,
        ApplyBoundary::ReplaceCurrent,
    ) {
        return Err(ApplyError::Replace {
            source,
            artifacts: Box::new(inspect_artifacts(
                &current_path,
                &staged.path,
                &rollback_path,
                &failed_replacement_path,
            )),
        });
    }

    let artifacts = inspect_artifacts(
        &current_path,
        &staged.path,
        &rollback_path,
        &failed_replacement_path,
    );
    if artifacts.current != ArtifactState::Fingerprint(staged.fingerprint)
        || artifacts.replacement != ArtifactState::Missing
        || artifacts.rollback != ArtifactState::Fingerprint(original_fingerprint)
        || artifacts.failed_replacement != ArtifactState::Missing
    {
        return Err(ApplyError::UnexpectedArtifacts(Box::new(artifacts)));
    }

    journal
        .set_phase(ApplyPhase::Replaced)
        .map_err(|source| journal_error("update", repository, &transaction_id, source))?;
    operations
        .publish(&mut journal_store, &journal, ApplyBoundary::PublishReplaced)
        .map_err(|source| journal_error("publish", repository, &transaction_id, source))?;
    let applied_fingerprint =
        match operations.validate(&current_path, ApplyBoundary::VerifyReplacement) {
            Ok(fingerprint) if fingerprint == staged.fingerprint => fingerprint,
            Ok(fingerprint) => {
                let reason = format!("expected {:?}, got {fingerprint:?}", staged.fingerprint);
                return rollback_verification_failure(
                    repository,
                    operations,
                    &mut journal,
                    journal_store,
                    &current_path,
                    &staged.path,
                    &rollback_path,
                    &failed_replacement_path,
                    original_fingerprint,
                    staged.fingerprint,
                    reason,
                );
            }
            Err(source) => {
                return rollback_verification_failure(
                    repository,
                    operations,
                    &mut journal,
                    journal_store,
                    &current_path,
                    &staged.path,
                    &rollback_path,
                    &failed_replacement_path,
                    original_fingerprint,
                    staged.fingerprint,
                    source.to_string(),
                );
            }
        };
    journal
        .set_phase(ApplyPhase::Verified)
        .map_err(|source| journal_error("update", repository, &transaction_id, source))?;
    operations
        .publish(&mut journal_store, &journal, ApplyBoundary::PublishVerified)
        .map_err(|source| journal_error("publish", repository, &transaction_id, source))?;
    remove_file_with(
        operations,
        &rollback_path,
        ApplyBoundary::CommitRollbackCleanup,
    )?;
    let journal_path = journal_store.path().to_path_buf();
    operations
        .remove(&journal_path, ApplyBoundary::CommitJournalCleanup)
        .map_err(|source| ApplyError::Cleanup {
            path: journal_path,
            source,
        })?;

    Ok(ApplyResult {
        automatic_stash,
        applied_fingerprint,
    })
}

#[cfg(test)]
fn is_game_running_before_replace() -> Result<bool, GameProcessError> {
    Ok(false)
}

#[cfg(not(test))]
fn is_game_running_before_replace() -> Result<bool, GameProcessError> {
    game_process::is_game_running()
}

#[allow(clippy::too_many_arguments)]
fn rollback_verification_failure(
    repository: &StoredSaveRepository,
    operations: &impl ApplyOperations,
    journal: &mut ApplyJournal,
    mut journal_store: JournalStore,
    current_path: &Path,
    replacement_path: &Path,
    rollback_path: &Path,
    failed_replacement_path: &Path,
    original_fingerprint: SaveFingerprint,
    replacement_fingerprint: SaveFingerprint,
    verification: String,
) -> Result<ApplyResult, ApplyError> {
    let rollback = (|| {
        journal
            .set_phase(ApplyPhase::RollingBack)
            .map_err(|source| {
                journal_error("update", repository, &journal.transaction_id, source)
            })?;
        operations
            .publish(
                &mut journal_store,
                journal,
                ApplyBoundary::PublishRollingBack,
            )
            .map_err(|source| {
                journal_error("publish", repository, &journal.transaction_id, source)
            })?;
        operations
            .replace(
                current_path,
                rollback_path,
                failed_replacement_path,
                ApplyBoundary::ReplaceRollback,
            )
            .map_err(|source| ApplyError::Replace {
                source,
                artifacts: Box::new(inspect_artifacts(
                    current_path,
                    replacement_path,
                    rollback_path,
                    failed_replacement_path,
                )),
            })?;
        let artifacts = inspect_artifacts(
            current_path,
            replacement_path,
            rollback_path,
            failed_replacement_path,
        );
        if artifacts.current != ArtifactState::Fingerprint(original_fingerprint)
            || artifacts.replacement != ArtifactState::Missing
            || artifacts.rollback != ArtifactState::Missing
            || artifacts.failed_replacement != ArtifactState::Fingerprint(replacement_fingerprint)
        {
            return Err(ApplyError::UnexpectedArtifacts(Box::new(artifacts)));
        }
        let restored = operations
            .validate(current_path, ApplyBoundary::VerifyRollback)
            .map_err(ApplyError::SaveFile)?;
        if restored != original_fingerprint {
            return Err(ApplyError::CurrentFingerprintChanged {
                expected: original_fingerprint,
                actual: restored,
            });
        }
        remove_file_with(
            operations,
            failed_replacement_path,
            ApplyBoundary::RollbackFailedCleanup,
        )?;
        let journal_path = journal_store.path().to_path_buf();
        operations
            .remove(&journal_path, ApplyBoundary::RollbackJournalCleanup)
            .map_err(|source| ApplyError::Cleanup {
                path: journal_path,
                source,
            })?;
        Ok(())
    })();

    match rollback {
        Ok(()) => Err(ApplyError::ReplacementVerificationRolledBack(verification)),
        Err(rollback) => Err(ApplyError::RollbackFailed {
            verification,
            rollback: Box::new(rollback),
        }),
    }
}

fn require_current(discovery: CurrentSaveDiscovery) -> Result<discovery::CurrentSave, ApplyError> {
    match discovery {
        CurrentSaveDiscovery::CurrentFound(current) => Ok(current),
        CurrentSaveDiscovery::SaveDirectoryMissing { directory } => {
            Err(ApplyError::SaveDirectoryMissing(directory))
        }
        CurrentSaveDiscovery::CurrentMissing { directory } => {
            Err(ApplyError::CurrentMissing(directory))
        }
    }
}

fn artifact_path(current: &Path, transaction_id: &str, kind: &str) -> Result<PathBuf, ApplyError> {
    let directory = current
        .parent()
        .ok_or_else(|| ApplyError::CurrentMissing(current.to_path_buf()))?;
    Ok(directory.join(format!("{ARTIFACT_PREFIX}{transaction_id}.{kind}.dat")))
}

fn require_missing(path: &Path) -> Result<(), ApplyError> {
    if path.try_exists().map_err(|source| ApplyError::Cleanup {
        path: path.to_path_buf(),
        source,
    })? {
        Err(ApplyError::ArtifactAlreadyExists(path.to_path_buf()))
    } else {
        Ok(())
    }
}

fn abort_before_replace(
    staged_path: PathBuf,
    journal_store: JournalStore,
) -> Result<(), ApplyError> {
    remove_file(&staged_path)?;
    let journal_path = journal_store.path().to_path_buf();
    journal_store
        .remove()
        .map_err(|source| ApplyError::Cleanup {
            path: journal_path,
            source,
        })
}

fn remove_file(path: &Path) -> Result<(), ApplyError> {
    fs::remove_file(path).map_err(|source| ApplyError::Cleanup {
        path: path.to_path_buf(),
        source,
    })
}

fn remove_file_with(
    operations: &impl ApplyOperations,
    path: &Path,
    boundary: ApplyBoundary,
) -> Result<(), ApplyError> {
    operations
        .remove(path, boundary)
        .map_err(|source| ApplyError::Cleanup {
            path: path.to_path_buf(),
            source,
        })
}

fn journal_error(
    operation: &'static str,
    repository: &StoredSaveRepository,
    transaction_id: &str,
    source: io::Error,
) -> ApplyError {
    ApplyError::Journal {
        operation,
        path: repository
            .root()
            .join("transactions")
            .join(format!("{transaction_id}.json")),
        source,
    }
}

fn inspect_artifacts(
    current: &Path,
    replacement: &Path,
    rollback: &Path,
    failed_replacement: &Path,
) -> ArtifactSnapshot {
    ArtifactSnapshot {
        current: inspect_artifact(current),
        replacement: inspect_artifact(replacement),
        rollback: inspect_artifact(rollback),
        failed_replacement: inspect_artifact(failed_replacement),
    }
}

fn inspect_artifact(path: &Path) -> ArtifactState {
    match path.try_exists() {
        Ok(false) => ArtifactState::Missing,
        Ok(true) => match validate_and_fingerprint(path) {
            Ok(fingerprint) => ArtifactState::Fingerprint(fingerprint),
            Err(error) => ArtifactState::Invalid(error.to_string()),
        },
        Err(error) => ArtifactState::Invalid(error.to_string()),
    }
}

impl From<DiscoveryError> for ApplyError {
    fn from(source: DiscoveryError) -> Self {
        Self::Discovery(source)
    }
}

impl From<SaveFileError> for ApplyError {
    fn from(source: SaveFileError) -> Self {
        Self::SaveFile(source)
    }
}

impl From<StorageError> for ApplyError {
    fn from(source: StorageError) -> Self {
        Self::Storage(source)
    }
}

impl From<StagingError> for ApplyError {
    fn from(source: StagingError) -> Self {
        Self::Staging(source)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use std::fs::{File, OpenOptions};
    use std::io::{Seek, SeekFrom, Write};
    use std::os::windows::fs::OpenOptionsExt;

    use tempfile::TempDir;

    use crate::discovery::save_directory_in;
    use crate::mutation_guard::MUTATION_GUARD_TEST;
    use crate::save_file::SAVE_FILE_SIZE;

    use super::*;

    struct FaultOperations {
        boundaries: Vec<ApplyBoundary>,
    }

    impl FaultOperations {
        fn new(boundaries: &[ApplyBoundary]) -> Self {
            Self {
                boundaries: boundaries.to_vec(),
            }
        }

        fn fail(&self, boundary: ApplyBoundary) -> io::Result<()> {
            if self.boundaries.contains(&boundary) {
                Err(io::Error::other("injected apply failure"))
            } else {
                Ok(())
            }
        }
    }

    impl ApplyOperations for FaultOperations {
        fn publish(
            &self,
            store: &mut JournalStore,
            journal: &ApplyJournal,
            boundary: ApplyBoundary,
        ) -> io::Result<()> {
            self.fail(boundary)?;
            SystemApplyOperations.publish(store, journal, boundary)
        }

        fn replace(
            &self,
            current: &Path,
            replacement: &Path,
            rollback: &Path,
            boundary: ApplyBoundary,
        ) -> io::Result<()> {
            self.fail(boundary)?;
            SystemApplyOperations.replace(current, replacement, rollback, boundary)
        }

        fn validate(
            &self,
            path: &Path,
            boundary: ApplyBoundary,
        ) -> Result<SaveFingerprint, SaveFileError> {
            self.fail(boundary).map_err(|source| SaveFileError::Read {
                path: path.to_path_buf(),
                source,
            })?;
            SystemApplyOperations.validate(path, boundary)
        }

        fn remove(&self, path: &Path, boundary: ApplyBoundary) -> io::Result<()> {
            self.fail(boundary)?;
            SystemApplyOperations.remove(path, boundary)
        }
    }

    struct ApplyTestContext {
        _directory: TempDir,
        repository: StoredSaveRepository,
        documents: PathBuf,
        current: PathBuf,
        replacement_id: String,
        original_fingerprint: SaveFingerprint,
        replacement_fingerprint: SaveFingerprint,
    }

    impl ApplyTestContext {
        fn new() -> Self {
            let directory = TempDir::new().unwrap();
            let repository = StoredSaveRepository::new(directory.path().join("app-data"));
            let replacement_source = directory.path().join("replacement.dat");
            create_save(&replacement_source, 2);
            let replacement = repository
                .capture(
                    &replacement_source,
                    StoredSaveKind::Preset,
                    "Practice".into(),
                    None,
                    StoredSaveOrigin::Imported,
                )
                .unwrap();
            let documents = directory.path().join("Documents");
            let save_directory = save_directory_in(&documents);
            fs::create_dir_all(&save_directory).unwrap();
            let current = save_directory.join("Vwings.dat");
            create_save(&current, 1);
            let original_fingerprint = validate_and_fingerprint(&current).unwrap();

            Self {
                _directory: directory,
                repository,
                documents,
                current,
                replacement_id: replacement.metadata.id,
                original_fingerprint,
                replacement_fingerprint: replacement.metadata.fingerprint,
            }
        }

        fn apply_with(&self, operations: &impl ApplyOperations) -> Result<ApplyResult, ApplyError> {
            apply_in_documents_with_operations(
                &self.repository,
                &self.documents,
                ApplyRequest {
                    stored_save_id: &self.replacement_id,
                    automatic_stash_alias: Some("Before Practice".into()),
                    automatic_stash_description: None,
                },
                || {},
                operations,
            )
        }
    }

    fn create_save(path: &Path, marker: u8) {
        let mut file = File::create(path).unwrap();
        file.set_len(SAVE_FILE_SIZE).unwrap();
        file.seek(SeekFrom::Start(SAVE_FILE_SIZE - 1)).unwrap();
        file.write_all(&[marker]).unwrap();
        file.sync_all().unwrap();
    }

    #[test]
    fn applies_a_stored_save_and_captures_current_as_a_stash() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let replacement_source = directory.path().join("replacement.dat");
        create_save(&replacement_source, 2);
        let replacement = repository
            .capture(
                &replacement_source,
                StoredSaveKind::Preset,
                "Practice".into(),
                None,
                StoredSaveOrigin::Imported,
            )
            .unwrap();
        let documents = directory.path().join("Documents");
        let save_directory = save_directory_in(&documents);
        fs::create_dir_all(&save_directory).unwrap();
        let current = save_directory.join("Vwings.dat");
        create_save(&current, 1);
        let original_fingerprint = validate_and_fingerprint(&current).unwrap();

        let result = apply_in_documents(
            &repository,
            &documents,
            ApplyRequest {
                stored_save_id: &replacement.metadata.id,
                automatic_stash_alias: Some("Before Practice".into()),
                automatic_stash_description: None,
            },
        )
        .unwrap();

        assert_eq!(replacement.metadata.fingerprint, result.applied_fingerprint);
        assert_eq!(
            replacement.metadata.fingerprint,
            validate_and_fingerprint(&current).unwrap()
        );
        assert_eq!(
            original_fingerprint,
            result.automatic_stash.metadata.fingerprint
        );
        assert_eq!(StoredSaveKind::Stash, result.automatic_stash.metadata.kind);
        assert_eq!("Vwings.dat", current.file_name().unwrap());
        assert!(
            fs::read_dir(repository.root().join("transactions"))
                .unwrap()
                .next()
                .is_none()
        );
        assert_eq!(2, repository.list().unwrap().len());
    }

    #[test]
    fn applies_a_save_without_duplicating_an_existing_verified_stash() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let replacement_source = directory.path().join("replacement.dat");
        create_save(&replacement_source, 2);
        let replacement = repository
            .capture(
                &replacement_source,
                StoredSaveKind::Preset,
                "Practice".into(),
                None,
                StoredSaveOrigin::Imported,
            )
            .unwrap();
        let documents = directory.path().join("Documents");
        let save_directory = save_directory_in(&documents);
        fs::create_dir_all(&save_directory).unwrap();
        let current = save_directory.join("Vwings.dat");
        create_save(&current, 1);
        let existing_stash = repository
            .capture(
                &current,
                StoredSaveKind::Stash,
                "Existing".into(),
                None,
                StoredSaveOrigin::Current,
            )
            .unwrap();

        let result = apply_in_documents(
            &repository,
            &documents,
            ApplyRequest {
                stored_save_id: &replacement.metadata.id,
                automatic_stash_alias: Some("Before Practice".into()),
                automatic_stash_description: None,
            },
        )
        .unwrap();

        assert!(!result.automatic_stash.created);
        assert_eq!(
            vec![existing_stash.metadata.id.clone()],
            result.automatic_stash.duplicate_ids
        );
        assert_eq!(existing_stash.metadata, result.automatic_stash.metadata);
        assert_eq!(2, repository.list().unwrap().len());
    }

    #[test]
    fn applies_a_save_without_stashing_current_that_matches_a_preset() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let current_source = directory.path().join("current-source.dat");
        create_save(&current_source, 1);
        let current_preset = repository
            .capture(
                &current_source,
                StoredSaveKind::Preset,
                "Current start".into(),
                None,
                StoredSaveOrigin::Imported,
            )
            .unwrap();
        let replacement_source = directory.path().join("replacement.dat");
        create_save(&replacement_source, 2);
        let replacement = repository
            .capture(
                &replacement_source,
                StoredSaveKind::Preset,
                "Next start".into(),
                None,
                StoredSaveOrigin::Imported,
            )
            .unwrap();
        let documents = directory.path().join("Documents");
        let save_directory = save_directory_in(&documents);
        fs::create_dir_all(&save_directory).unwrap();
        let current = save_directory.join("Vwings.dat");
        create_save(&current, 1);

        let result = apply_in_documents(
            &repository,
            &documents,
            ApplyRequest {
                stored_save_id: &replacement.metadata.id,
                automatic_stash_alias: None,
                automatic_stash_description: None,
            },
        )
        .unwrap();

        assert!(!result.automatic_stash.created);
        assert_eq!(current_preset.metadata, result.automatic_stash.metadata);
        assert_eq!(2, repository.list().unwrap().len());
    }

    #[test]
    fn aborts_before_replacement_when_current_changes() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let replacement_source = directory.path().join("replacement.dat");
        create_save(&replacement_source, 2);
        let replacement = repository
            .capture(
                &replacement_source,
                StoredSaveKind::Preset,
                "Practice".into(),
                None,
                StoredSaveOrigin::Imported,
            )
            .unwrap();
        let documents = directory.path().join("Documents");
        let save_directory = save_directory_in(&documents);
        fs::create_dir_all(&save_directory).unwrap();
        let current = save_directory.join("Vwings.dat");
        create_save(&current, 1);

        let result = apply_in_documents_with_before_recheck(
            &repository,
            &documents,
            ApplyRequest {
                stored_save_id: &replacement.metadata.id,
                automatic_stash_alias: Some("Before Practice".into()),
                automatic_stash_description: None,
            },
            || create_save(&current, 3),
        );

        assert!(matches!(
            result,
            Err(ApplyError::CurrentFingerprintChanged { .. })
        ));
        assert_ne!(
            replacement.metadata.fingerprint,
            validate_and_fingerprint(&current).unwrap()
        );
        assert!(fs::read_dir(&save_directory).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(ARTIFACT_PREFIX)
        }));
        assert!(
            fs::read_dir(repository.root().join("transactions"))
                .unwrap()
                .next()
                .is_none()
        );
        assert_eq!(2, repository.list().unwrap().len());
    }

    #[test]
    fn restores_original_current_when_replacement_verification_fails() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let context = ApplyTestContext::new();
        let operations = FaultOperations::new(&[ApplyBoundary::VerifyReplacement]);

        let result = context.apply_with(&operations);

        assert!(matches!(
            result,
            Err(ApplyError::ReplacementVerificationRolledBack(_))
        ));
        assert_eq!(
            context.original_fingerprint,
            validate_and_fingerprint(&context.current).unwrap()
        );
        assert!(
            unfinished_journals(context.repository.root())
                .unwrap()
                .is_empty()
        );
        assert!(
            fs::read_dir(context.current.parent().unwrap())
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(ARTIFACT_PREFIX))
        );
    }

    #[test]
    fn startup_recovery_cleans_an_injected_replacement_failure() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let context = ApplyTestContext::new();
        let operations = FaultOperations::new(&[ApplyBoundary::ReplaceCurrent]);

        let result = context.apply_with(&operations);

        assert!(matches!(result, Err(ApplyError::Replace { .. })));
        assert_eq!(
            context.original_fingerprint,
            validate_and_fingerprint(&context.current).unwrap()
        );
        let recovered = crate::recovery::recover_unfinished_transactions_in_documents(
            &context.repository,
            &context.documents,
        )
        .unwrap();
        assert_eq!(
            crate::recovery::RecoveryAction::AbortedReplacement,
            recovered[0].action
        );
    }

    #[test]
    fn startup_recovery_cleans_a_real_current_sharing_violation() {
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let context = ApplyTestContext::new();
        let current_lock = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&context.current)
            .unwrap();

        let result = context.apply_with(&SystemApplyOperations);

        assert!(matches!(result, Err(ApplyError::Replace { .. })));
        assert_eq!(
            context.original_fingerprint,
            validate_and_fingerprint(&context.current).unwrap()
        );
        drop(current_lock);

        let recovered = crate::recovery::recover_unfinished_transactions_in_documents(
            &context.repository,
            &context.documents,
        )
        .unwrap();
        assert_eq!(
            crate::recovery::RecoveryAction::AbortedReplacement,
            recovered[0].action
        );
        assert_eq!(
            context.original_fingerprint,
            validate_and_fingerprint(&context.current).unwrap()
        );
    }

    #[test]
    fn startup_recovery_resolves_an_injected_rollback_failure() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let context = ApplyTestContext::new();
        let operations = FaultOperations::new(&[
            ApplyBoundary::VerifyReplacement,
            ApplyBoundary::ReplaceRollback,
        ]);

        let result = context.apply_with(&operations);

        assert!(matches!(result, Err(ApplyError::RollbackFailed { .. })));
        let recovered = crate::recovery::recover_unfinished_transactions_in_documents(
            &context.repository,
            &context.documents,
        )
        .unwrap();
        assert_eq!(
            crate::recovery::RecoveryAction::FinishedReplacement,
            recovered[0].action
        );
        assert_eq!(
            context.replacement_fingerprint,
            validate_and_fingerprint(&context.current).unwrap()
        );
    }

    #[test]
    fn startup_recovery_finishes_each_injected_commit_cleanup_failure() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        for (boundary, expected_action) in [
            (
                ApplyBoundary::CommitRollbackCleanup,
                crate::recovery::RecoveryAction::FinishedReplacement,
            ),
            (
                ApplyBoundary::CommitJournalCleanup,
                crate::recovery::RecoveryAction::FinishedVerifiedCleanup,
            ),
        ] {
            let context = ApplyTestContext::new();
            let operations = FaultOperations::new(&[boundary]);

            let result = context.apply_with(&operations);

            assert!(matches!(result, Err(ApplyError::Cleanup { .. })));
            let recovered = crate::recovery::recover_unfinished_transactions_in_documents(
                &context.repository,
                &context.documents,
            )
            .unwrap();
            assert_eq!(expected_action, recovered[0].action);
            assert_eq!(
                context.replacement_fingerprint,
                validate_and_fingerprint(&context.current).unwrap()
            );
        }
    }

    #[test]
    fn startup_recovery_cleans_an_injected_journal_update_failure() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let context = ApplyTestContext::new();
        let operations = FaultOperations::new(&[ApplyBoundary::PublishReplacing]);

        let result = context.apply_with(&operations);

        assert!(matches!(result, Err(ApplyError::Journal { .. })));
        let recovered = crate::recovery::recover_unfinished_transactions_in_documents(
            &context.repository,
            &context.documents,
        )
        .unwrap();
        assert_eq!(
            crate::recovery::RecoveryAction::AbortedReplacement,
            recovered[0].action
        );
        assert_eq!(
            context.original_fingerprint,
            validate_and_fingerprint(&context.current).unwrap()
        );
    }
}
