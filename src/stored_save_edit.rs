use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use crate::mutation_guard::{MutationGuard, MutationGuardError};
use crate::recovery::{RecoveryError, unfinished_journals};
use crate::storage::{StorageError, StoredSaveRepository};
use crate::stored_save::StoredSaveMetadata;

pub struct EditStoredSaveRequest<'a> {
    pub stored_save_id: &'a str,
    pub alias: String,
    pub description: Option<String>,
}

#[derive(Debug)]
pub enum StoredSaveEditError {
    MutationGuard(MutationGuardError),
    Recovery(RecoveryError),
    RecoveryRequired(Vec<PathBuf>),
    Storage(StorageError),
}

impl fmt::Display for StoredSaveEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MutationGuard(source) => {
                write!(formatter, "metadata update is blocked: {source}")
            }
            Self::Recovery(source) => write!(formatter, "transaction scan failed: {source}"),
            Self::RecoveryRequired(paths) => write!(
                formatter,
                "unfinished transaction recovery is required before metadata update: {paths:?}"
            ),
            Self::Storage(source) => {
                write!(formatter, "StoredSave metadata update failed: {source}")
            }
        }
    }
}

impl Error for StoredSaveEditError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MutationGuard(source) => Some(source),
            Self::Recovery(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::RecoveryRequired(_) => None,
        }
    }
}

pub fn promote_stash_to_preset(
    repository: &StoredSaveRepository,
    stored_save_id: &str,
) -> Result<StoredSaveMetadata, StoredSaveEditError> {
    with_edit_guard(repository, || repository.promote_to_preset(stored_save_id))
}

pub fn edit_stored_save(
    repository: &StoredSaveRepository,
    request: EditStoredSaveRequest<'_>,
) -> Result<StoredSaveMetadata, StoredSaveEditError> {
    with_edit_guard(repository, || {
        repository.update_details(request.stored_save_id, request.alias, request.description)
    })
}

fn with_edit_guard(
    repository: &StoredSaveRepository,
    edit: impl FnOnce() -> Result<StoredSaveMetadata, StorageError>,
) -> Result<StoredSaveMetadata, StoredSaveEditError> {
    let _guard = MutationGuard::acquire().map_err(StoredSaveEditError::MutationGuard)?;
    let unfinished =
        unfinished_journals(repository.root()).map_err(StoredSaveEditError::Recovery)?;
    if !unfinished.is_empty() {
        return Err(StoredSaveEditError::RecoveryRequired(unfinished));
    }
    edit().map_err(StoredSaveEditError::Storage)
}

#[cfg(all(test, windows))]
mod tests {
    use std::fs::{self, File};
    use std::io::{Seek, SeekFrom, Write};
    use std::path::Path;

    use tempfile::TempDir;
    use uuid::Uuid;

    use crate::mutation_guard::MUTATION_GUARD_TEST;
    use crate::save_file::SAVE_FILE_SIZE;
    use crate::stored_save::{StoredSaveKind, StoredSaveOrigin};
    use crate::transaction::TRANSACTIONS_DIRECTORY_NAME;

    use super::*;

    fn create_save(path: &Path) {
        let mut file = File::create(path).unwrap();
        file.set_len(SAVE_FILE_SIZE).unwrap();
        file.seek(SeekFrom::Start(SAVE_FILE_SIZE - 1)).unwrap();
        file.write_all(&[1]).unwrap();
        file.sync_all().unwrap();
    }

    #[test]
    fn promotes_and_edits_a_stored_save_through_the_guarded_operations() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let source = directory.path().join("Vwings.dat");
        create_save(&source);
        let captured = repository
            .capture(
                &source,
                StoredSaveKind::Stash,
                "Recovery".into(),
                None,
                StoredSaveOrigin::Current,
            )
            .unwrap();

        let promoted = promote_stash_to_preset(&repository, &captured.metadata.id).unwrap();
        let edited = edit_stored_save(
            &repository,
            EditStoredSaveRequest {
                stored_save_id: &captured.metadata.id,
                alias: "Practice".into(),
                description: Some("Chapter start".into()),
            },
        )
        .unwrap();

        assert_eq!(StoredSaveKind::Preset, promoted.kind);
        assert_eq!("Practice", edited.alias);
        assert_eq!(Some("Chapter start"), edited.description.as_deref());
        assert_eq!(captured.metadata.fingerprint, edited.fingerprint);
    }

    #[test]
    fn unfinished_transaction_blocks_metadata_updates() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let source = directory.path().join("Vwings.dat");
        create_save(&source);
        let captured = repository
            .capture(
                &source,
                StoredSaveKind::Stash,
                "Recovery".into(),
                None,
                StoredSaveOrigin::Current,
            )
            .unwrap();
        let transactions = repository.root().join(TRANSACTIONS_DIRECTORY_NAME);
        fs::create_dir_all(&transactions).unwrap();
        let journal_path = transactions.join(format!("{}.json", Uuid::new_v4()));
        fs::write(&journal_path, b"unfinished").unwrap();

        let result = promote_stash_to_preset(&repository, &captured.metadata.id);

        assert!(matches!(
            result,
            Err(StoredSaveEditError::RecoveryRequired(paths)) if paths == vec![journal_path]
        ));
        assert_eq!(StoredSaveKind::Stash, repository.list().unwrap()[0].kind);
    }
}
