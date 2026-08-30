use std::error::Error;
use std::fmt;

use crate::alias::AliasError;
use crate::apply::ApplyError;
use crate::built_in::BuiltInPresetError;
use crate::current_capture::CaptureCurrentError;
use crate::discovery::DiscoveryError;
use crate::first_activation::FirstActivationError;
use crate::game_process::GameProcessError;
use crate::import_save::ImportSaveError;
use crate::known_folders::KnownFolderError;
use crate::mutation_guard::MutationGuardError;
use crate::operation_lock::OperationLockError;
use crate::recovery::RecoveryError;
use crate::save_file::SaveFileError;
use crate::staging::StagingError;
use crate::storage::StorageError;
use crate::stored_save_delete::StoredSaveDeleteError;
use crate::stored_save_edit::StoredSaveEditError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationOperation {
    Apply,
    ManageBuiltInPreset,
    CaptureCurrent,
    ImportSave,
    EditStoredSave,
    DeleteStoredSave,
    ActivateCurrent,
    RecoverTransactions,
    DiscoverCurrent,
    AccessStoredSaves,
    LocateApplicationData,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAction {
    CorrectAlias,
    SelectValidImport,
    CloseGame,
    WaitForOtherOperation,
    RecoverTransactions,
    ResolveRecoveryManually,
    CreateSaveDirectory,
    ActivateCurrent,
    RefreshCurrent,
    CheckFileAccess,
    ResolveStoredSaveProblem,
    SelectStash,
    ConfirmCurrentFilename,
    UseSupportedPlatform,
    Retry,
    ReportProblem,
    CorrectDescription,
}

#[derive(Debug)]
pub struct ApplicationError {
    operation: ApplicationOperation,
    action: UserAction,
    detail: ApplicationErrorDetail,
}

impl ApplicationError {
    pub fn operation(&self) -> ApplicationOperation {
        self.operation
    }

    pub fn action(&self) -> UserAction {
        self.action
    }

    pub fn detail(&self) -> &ApplicationErrorDetail {
        &self.detail
    }

    pub fn into_detail(self) -> ApplicationErrorDetail {
        self.detail
    }
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.detail.fmt(formatter)
    }
}

impl Error for ApplicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.detail)
    }
}

#[derive(Debug)]
pub enum ApplicationErrorDetail {
    Apply(Box<ApplyError>),
    BuiltInPreset(Box<BuiltInPresetError>),
    CaptureCurrent(Box<CaptureCurrentError>),
    ImportSave(Box<ImportSaveError>),
    EditStoredSave(Box<StoredSaveEditError>),
    DeleteStoredSave(Box<StoredSaveDeleteError>),
    FirstActivation(Box<FirstActivationError>),
    Recovery(Box<RecoveryError>),
    Discovery(Box<DiscoveryError>),
    Storage(Box<StorageError>),
    KnownFolder(Box<KnownFolderError>),
}

impl fmt::Display for ApplicationErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Apply(source) => source.fmt(formatter),
            Self::BuiltInPreset(source) => source.fmt(formatter),
            Self::CaptureCurrent(source) => source.fmt(formatter),
            Self::ImportSave(source) => source.fmt(formatter),
            Self::EditStoredSave(source) => source.fmt(formatter),
            Self::DeleteStoredSave(source) => source.fmt(formatter),
            Self::FirstActivation(source) => source.fmt(formatter),
            Self::Recovery(source) => source.fmt(formatter),
            Self::Discovery(source) => source.fmt(formatter),
            Self::Storage(source) => source.fmt(formatter),
            Self::KnownFolder(source) => source.fmt(formatter),
        }
    }
}

impl Error for ApplicationErrorDetail {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Apply(source) => Some(source.as_ref()),
            Self::BuiltInPreset(source) => Some(source.as_ref()),
            Self::CaptureCurrent(source) => Some(source.as_ref()),
            Self::ImportSave(source) => Some(source.as_ref()),
            Self::EditStoredSave(source) => Some(source.as_ref()),
            Self::DeleteStoredSave(source) => Some(source.as_ref()),
            Self::FirstActivation(source) => Some(source.as_ref()),
            Self::Recovery(source) => Some(source.as_ref()),
            Self::Discovery(source) => Some(source.as_ref()),
            Self::Storage(source) => Some(source.as_ref()),
            Self::KnownFolder(source) => Some(source.as_ref()),
        }
    }
}

impl From<ApplyError> for ApplicationError {
    fn from(source: ApplyError) -> Self {
        Self {
            operation: ApplicationOperation::Apply,
            action: classify_apply(&source),
            detail: ApplicationErrorDetail::Apply(Box::new(source)),
        }
    }
}

impl From<BuiltInPresetError> for ApplicationError {
    fn from(source: BuiltInPresetError) -> Self {
        Self {
            operation: ApplicationOperation::ManageBuiltInPreset,
            action: classify_built_in_preset(&source),
            detail: ApplicationErrorDetail::BuiltInPreset(Box::new(source)),
        }
    }
}

impl From<CaptureCurrentError> for ApplicationError {
    fn from(source: CaptureCurrentError) -> Self {
        Self {
            operation: ApplicationOperation::CaptureCurrent,
            action: classify_capture_current(&source),
            detail: ApplicationErrorDetail::CaptureCurrent(Box::new(source)),
        }
    }
}

impl From<ImportSaveError> for ApplicationError {
    fn from(source: ImportSaveError) -> Self {
        Self {
            operation: ApplicationOperation::ImportSave,
            action: classify_import(&source),
            detail: ApplicationErrorDetail::ImportSave(Box::new(source)),
        }
    }
}

impl From<StoredSaveEditError> for ApplicationError {
    fn from(source: StoredSaveEditError) -> Self {
        Self {
            operation: ApplicationOperation::EditStoredSave,
            action: classify_stored_save_edit(&source),
            detail: ApplicationErrorDetail::EditStoredSave(Box::new(source)),
        }
    }
}

impl From<StoredSaveDeleteError> for ApplicationError {
    fn from(source: StoredSaveDeleteError) -> Self {
        let action = match &source {
            StoredSaveDeleteError::MutationGuard(source) => classify_mutation_guard(source),
            StoredSaveDeleteError::Recovery(source) => classify_recovery(source),
            StoredSaveDeleteError::RecoveryRequired(_) => UserAction::RecoverTransactions,
            StoredSaveDeleteError::Storage(source) => {
                classify_storage(source, SaveFileContext::StoredSave)
            }
        };
        Self {
            operation: ApplicationOperation::DeleteStoredSave,
            action,
            detail: ApplicationErrorDetail::DeleteStoredSave(Box::new(source)),
        }
    }
}

impl From<FirstActivationError> for ApplicationError {
    fn from(source: FirstActivationError) -> Self {
        Self {
            operation: ApplicationOperation::ActivateCurrent,
            action: classify_first_activation(&source),
            detail: ApplicationErrorDetail::FirstActivation(Box::new(source)),
        }
    }
}

impl From<RecoveryError> for ApplicationError {
    fn from(source: RecoveryError) -> Self {
        Self {
            operation: ApplicationOperation::RecoverTransactions,
            action: classify_recovery(&source),
            detail: ApplicationErrorDetail::Recovery(Box::new(source)),
        }
    }
}

impl From<DiscoveryError> for ApplicationError {
    fn from(source: DiscoveryError) -> Self {
        Self {
            operation: ApplicationOperation::DiscoverCurrent,
            action: classify_discovery(&source),
            detail: ApplicationErrorDetail::Discovery(Box::new(source)),
        }
    }
}

impl From<StorageError> for ApplicationError {
    fn from(source: StorageError) -> Self {
        Self {
            operation: ApplicationOperation::AccessStoredSaves,
            action: classify_storage(&source, SaveFileContext::StoredSave),
            detail: ApplicationErrorDetail::Storage(Box::new(source)),
        }
    }
}

impl From<KnownFolderError> for ApplicationError {
    fn from(source: KnownFolderError) -> Self {
        Self {
            operation: ApplicationOperation::LocateApplicationData,
            action: classify_known_folder(&source),
            detail: ApplicationErrorDetail::KnownFolder(Box::new(source)),
        }
    }
}

#[derive(Clone, Copy)]
enum SaveFileContext {
    Current,
    Import,
    StoredSave,
}

fn classify_apply(error: &ApplyError) -> UserAction {
    match error {
        ApplyError::Alias(_) => UserAction::CorrectAlias,
        ApplyError::MutationGuard(source) => classify_mutation_guard(source),
        ApplyError::Recovery(source) => classify_recovery(source),
        ApplyError::RecoveryRequired(_) => UserAction::RecoverTransactions,
        ApplyError::Discovery(source) => classify_discovery(source),
        ApplyError::SaveDirectoryMissing(_) => UserAction::CreateSaveDirectory,
        ApplyError::CurrentMissing(_) => UserAction::ActivateCurrent,
        ApplyError::CurrentPathChanged { .. } | ApplyError::CurrentFingerprintChanged { .. } => {
            UserAction::RefreshCurrent
        }
        ApplyError::GameProcess(source) => classify_game_process(source),
        ApplyError::GameRunning => UserAction::CloseGame,
        ApplyError::SaveFile(source) => classify_save_file(source, SaveFileContext::Current),
        ApplyError::Storage(source) => classify_storage(source, SaveFileContext::StoredSave),
        ApplyError::Staging(source) => classify_staging(source),
        ApplyError::ArtifactAlreadyExists(_) | ApplyError::UnexpectedArtifacts(_) => {
            UserAction::ResolveRecoveryManually
        }
        ApplyError::Journal { .. } | ApplyError::Replace { .. } | ApplyError::Cleanup { .. } => {
            UserAction::RecoverTransactions
        }
        ApplyError::ReplacementVerificationRolledBack(_) => UserAction::CheckFileAccess,
        ApplyError::RollbackFailed { .. } => UserAction::ResolveRecoveryManually,
    }
}

fn classify_built_in_preset(error: &BuiltInPresetError) -> UserAction {
    match error {
        BuiltInPresetError::MutationGuard(source) => classify_mutation_guard(source),
        BuiltInPresetError::Recovery(source) => classify_recovery(source),
        BuiltInPresetError::RecoveryRequired(_) => UserAction::RecoverTransactions,
        BuiltInPresetError::Storage(source) => {
            classify_storage(source, SaveFileContext::StoredSave)
        }
    }
}

fn classify_capture_current(error: &CaptureCurrentError) -> UserAction {
    match error {
        CaptureCurrentError::Alias(_) => UserAction::CorrectAlias,
        CaptureCurrentError::MutationGuard(source) => classify_mutation_guard(source),
        CaptureCurrentError::Recovery(source) => classify_recovery(source),
        CaptureCurrentError::RecoveryRequired(_) => UserAction::RecoverTransactions,
        CaptureCurrentError::Discovery(source) => classify_discovery(source),
        CaptureCurrentError::SaveDirectoryMissing(_) => UserAction::CreateSaveDirectory,
        CaptureCurrentError::CurrentMissing(_) => UserAction::ActivateCurrent,
        CaptureCurrentError::Storage(source) => classify_storage(source, SaveFileContext::Current),
    }
}

fn classify_import(error: &ImportSaveError) -> UserAction {
    match error {
        ImportSaveError::Alias(_) => UserAction::CorrectAlias,
        ImportSaveError::MutationGuard(source) => classify_mutation_guard(source),
        ImportSaveError::Recovery(source) => classify_recovery(source),
        ImportSaveError::RecoveryRequired(_) => UserAction::RecoverTransactions,
        ImportSaveError::InvalidExtension(_) => UserAction::SelectValidImport,
        ImportSaveError::Storage(source) => classify_storage(source, SaveFileContext::Import),
    }
}

fn classify_stored_save_edit(error: &StoredSaveEditError) -> UserAction {
    match error {
        StoredSaveEditError::MutationGuard(source) => classify_mutation_guard(source),
        StoredSaveEditError::Recovery(source) => classify_recovery(source),
        StoredSaveEditError::RecoveryRequired(_) => UserAction::RecoverTransactions,
        StoredSaveEditError::Storage(source) => {
            classify_storage(source, SaveFileContext::StoredSave)
        }
    }
}

fn classify_first_activation(error: &FirstActivationError) -> UserAction {
    match error {
        FirstActivationError::AccountName(_) => UserAction::Retry,
        FirstActivationError::InvalidUnicode | FirstActivationError::InvalidFilenameStem(_) => {
            UserAction::ReportProblem
        }
        FirstActivationError::FilenameNotConfirmed { .. } => UserAction::ConfirmCurrentFilename,
        FirstActivationError::MutationGuard(source) => classify_mutation_guard(source),
        FirstActivationError::Recovery(source) => classify_recovery(source),
        FirstActivationError::RecoveryRequired(_) => UserAction::RecoverTransactions,
        FirstActivationError::Discovery(source) => classify_discovery(source),
        FirstActivationError::SaveDirectoryMissing(_) => UserAction::CreateSaveDirectory,
        FirstActivationError::CurrentAlreadyExists(_) => UserAction::RefreshCurrent,
        FirstActivationError::GameProcess(source) => classify_game_process(source),
        FirstActivationError::GameRunning => UserAction::CloseGame,
        FirstActivationError::Staging(source) => classify_staging(source),
        FirstActivationError::Journal { .. }
        | FirstActivationError::Publish { .. }
        | FirstActivationError::SaveFile(_)
        | FirstActivationError::FingerprintMismatch { .. }
        | FirstActivationError::Cleanup { .. } => UserAction::RecoverTransactions,
    }
}

fn classify_recovery(error: &RecoveryError) -> UserAction {
    match error {
        RecoveryError::KnownFolder(source) => classify_known_folder(source),
        RecoveryError::MutationGuard(source) => classify_mutation_guard(source),
        RecoveryError::Scan { .. } | RecoveryError::FileOperation { .. } => {
            UserAction::CheckFileAccess
        }
        RecoveryError::InvalidJournal { .. } | RecoveryError::Blocked { .. } => {
            UserAction::ResolveRecoveryManually
        }
    }
}

fn classify_discovery(error: &DiscoveryError) -> UserAction {
    match error {
        DiscoveryError::KnownFolder(source) => classify_known_folder(source),
        DiscoveryError::CurrentFilename(source) => classify_first_activation(source),
        DiscoveryError::ReadDirectory { .. } | DiscoveryError::InspectCurrent { .. } => {
            UserAction::CheckFileAccess
        }
        DiscoveryError::InvalidSaveDirectory(_) | DiscoveryError::InvalidCurrentType(_) => {
            UserAction::ReportProblem
        }
    }
}

fn classify_mutation_guard(error: &MutationGuardError) -> UserAction {
    match error {
        MutationGuardError::OperationInProgress => UserAction::WaitForOtherOperation,
        MutationGuardError::OperationLock(source) => classify_operation_lock(source),
        MutationGuardError::GameProcess(source) => classify_game_process(source),
        MutationGuardError::GameRunning => UserAction::CloseGame,
    }
}

fn classify_operation_lock(error: &OperationLockError) -> UserAction {
    match error {
        OperationLockError::Create(_) | OperationLockError::Wait(_) => UserAction::Retry,
        OperationLockError::UnexpectedWaitStatus(_) => UserAction::ReportProblem,
        OperationLockError::UnsupportedPlatform => UserAction::UseSupportedPlatform,
    }
}

fn classify_game_process(error: &GameProcessError) -> UserAction {
    match error {
        GameProcessError::Snapshot(_) | GameProcessError::Enumerate(_) => UserAction::Retry,
        GameProcessError::UnsupportedPlatform => UserAction::UseSupportedPlatform,
    }
}

fn classify_known_folder(error: &KnownFolderError) -> UserAction {
    match error {
        KnownFolderError::Windows(_) => UserAction::Retry,
        KnownFolderError::EmptyPath => UserAction::ReportProblem,
        KnownFolderError::UnsupportedPlatform => UserAction::UseSupportedPlatform,
    }
}

fn classify_staging(error: &StagingError) -> UserAction {
    match error {
        StagingError::InvalidTransactionId(_) | StagingError::CurrentWithoutParent(_) => {
            UserAction::ReportProblem
        }
        StagingError::Storage(source) => classify_storage(source, SaveFileContext::StoredSave),
    }
}

fn classify_storage(error: &StorageError, context: SaveFileContext) -> UserAction {
    match error {
        StorageError::Alias(source) => classify_alias(source),
        StorageError::Description(_) => UserAction::CorrectDescription,
        StorageError::Source(source) => classify_save_file(source, context),
        StorageError::Io { .. } => UserAction::CheckFileAccess,
        StorageError::Json { .. }
        | StorageError::InvalidMetadata { .. }
        | StorageError::PayloadMismatch { .. } => UserAction::ResolveStoredSaveProblem,
        StorageError::UnknownBuiltIn(_) | StorageError::BuiltInImmutable(_) => {
            UserAction::ResolveStoredSaveProblem
        }
        StorageError::NotAStash(_) => UserAction::SelectStash,
        StorageError::InvalidTimestamp => UserAction::ReportProblem,
    }
}

fn classify_alias(_error: &AliasError) -> UserAction {
    UserAction::CorrectAlias
}

fn classify_save_file(error: &SaveFileError, context: SaveFileContext) -> UserAction {
    match context {
        SaveFileContext::Import => UserAction::SelectValidImport,
        SaveFileContext::StoredSave => UserAction::ResolveStoredSaveProblem,
        SaveFileContext::Current => match error {
            SaveFileError::Open { .. }
            | SaveFileError::Metadata { .. }
            | SaveFileError::Read { .. } => UserAction::CheckFileAccess,
            SaveFileError::NotAFile { .. } | SaveFileError::UnexpectedSize { .. } => {
                UserAction::RefreshCurrent
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn user_action_codes_match_the_ui_guidance_boundary() {
        assert_eq!(0, UserAction::CorrectAlias as i32);
        assert_eq!(11, UserAction::SelectStash as i32);
        assert_eq!(15, UserAction::ReportProblem as i32);
        assert_eq!(16, UserAction::CorrectDescription as i32);
    }

    #[test]
    fn classifies_common_operation_blocks() {
        let game_running =
            ApplicationError::from(ApplyError::MutationGuard(MutationGuardError::GameRunning));
        assert_eq!(game_running.operation(), ApplicationOperation::Apply);
        assert_eq!(game_running.action(), UserAction::CloseGame);

        let operation_running = ApplicationError::from(CaptureCurrentError::MutationGuard(
            MutationGuardError::OperationInProgress,
        ));
        assert_eq!(
            operation_running.action(),
            UserAction::WaitForOtherOperation
        );

        let recovery_required =
            ApplicationError::from(ImportSaveError::RecoveryRequired(vec![PathBuf::from(
                "transaction.json",
            )]));
        assert_eq!(recovery_required.action(), UserAction::RecoverTransactions);
    }

    #[test]
    fn distinguishes_recoverable_and_manual_transaction_states() {
        let journal_failure = ApplicationError::from(ApplyError::Journal {
            operation: "publish",
            path: PathBuf::from("transaction.json"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        });
        assert_eq!(journal_failure.action(), UserAction::RecoverTransactions);

        let blocked = ApplicationError::from(RecoveryError::Blocked {
            journal_path: PathBuf::from("transaction.json"),
            reason: "contradictory fingerprints".into(),
        });
        assert_eq!(
            blocked.operation(),
            ApplicationOperation::RecoverTransactions
        );
        assert_eq!(blocked.action(), UserAction::ResolveRecoveryManually);

        let rollback_failed = ApplicationError::from(ApplyError::RollbackFailed {
            verification: "replacement mismatch".into(),
            rollback: Box::new(ApplyError::Cleanup {
                path: PathBuf::from("rollback.dat"),
                source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
            }),
        });
        assert_eq!(
            rollback_failed.action(),
            UserAction::ResolveRecoveryManually
        );
    }

    #[test]
    fn classifies_file_errors_by_operation_context() {
        let invalid_import = ApplicationError::from(ImportSaveError::Storage(
            StorageError::Source(SaveFileError::UnexpectedSize {
                path: PathBuf::from("short.dat"),
                expected: 10,
                actual: 2,
            }),
        ));
        assert_eq!(invalid_import.action(), UserAction::SelectValidImport);

        let invalid_current = ApplicationError::from(CaptureCurrentError::Storage(
            StorageError::Source(SaveFileError::UnexpectedSize {
                path: PathBuf::from("Current.dat"),
                expected: 10,
                actual: 2,
            }),
        ));
        assert_eq!(invalid_current.action(), UserAction::RefreshCurrent);

        let corrupt_payload = ApplicationError::from(StorageError::PayloadMismatch {
            path: PathBuf::from("payload.dat.gz"),
            expected: fingerprint(1),
            actual: fingerprint(2),
        });
        assert_eq!(
            corrupt_payload.action(),
            UserAction::ResolveStoredSaveProblem
        );
    }

    #[test]
    fn preserves_diagnostic_detail_and_source_chain() {
        let source = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let error = ApplicationError::from(ImportSaveError::Storage(StorageError::Io {
            operation: "create",
            path: PathBuf::from("payload.dat.gz"),
            source,
        }));

        assert_eq!(error.action(), UserAction::CheckFileAccess);
        assert!(error.to_string().contains("payload.dat.gz"));
        assert!(error.source().is_some());
        assert!(error.source().unwrap().source().is_some());
        assert!(matches!(
            error.detail(),
            ApplicationErrorDetail::ImportSave(_)
        ));
    }

    #[test]
    fn classifies_activation_and_platform_guidance() {
        let confirmation = ApplicationError::from(FirstActivationError::FilenameNotConfirmed {
            expected: "User.dat".into(),
            actual: "other.dat".into(),
        });
        assert_eq!(confirmation.action(), UserAction::ConfirmCurrentFilename);

        let unsupported = ApplicationError::from(KnownFolderError::UnsupportedPlatform);
        assert_eq!(
            unsupported.operation(),
            ApplicationOperation::LocateApplicationData
        );
        assert_eq!(unsupported.action(), UserAction::UseSupportedPlatform);
    }

    fn fingerprint(byte: u8) -> crate::save_file::SaveFingerprint {
        crate::save_file::SaveFingerprint {
            size: 9_134_256,
            sha256: crate::save_file::SaveHash::from_bytes([byte; 32]),
        }
    }
}
