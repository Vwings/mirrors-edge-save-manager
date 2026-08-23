use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use crate::mutation_guard::{MutationGuard, MutationGuardError};
use crate::recovery::{RecoveryError, unfinished_journals};
use crate::storage::{StorageError, StoredSaveRepository};

#[derive(Debug)]
pub enum StoredSaveDeleteError {
    MutationGuard(MutationGuardError),
    Recovery(RecoveryError),
    RecoveryRequired(Vec<PathBuf>),
    Storage(StorageError),
}

impl fmt::Display for StoredSaveDeleteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MutationGuard(source) => write!(formatter, "deletion is blocked: {source}"),
            Self::Recovery(source) => write!(formatter, "transaction scan failed: {source}"),
            Self::RecoveryRequired(paths) => write!(
                formatter,
                "unfinished transaction recovery is required before deletion: {paths:?}"
            ),
            Self::Storage(source) => write!(formatter, "StoredSave deletion failed: {source}"),
        }
    }
}

impl Error for StoredSaveDeleteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MutationGuard(source) => Some(source),
            Self::Recovery(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::RecoveryRequired(_) => None,
        }
    }
}

pub fn delete_stored_save(
    repository: &StoredSaveRepository,
    stored_save_id: &str,
) -> Result<(), StoredSaveDeleteError> {
    let _guard = MutationGuard::acquire().map_err(StoredSaveDeleteError::MutationGuard)?;
    let unfinished =
        unfinished_journals(repository.root()).map_err(StoredSaveDeleteError::Recovery)?;
    if !unfinished.is_empty() {
        return Err(StoredSaveDeleteError::RecoveryRequired(unfinished));
    }
    repository
        .delete(stored_save_id)
        .map_err(StoredSaveDeleteError::Storage)
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
    fn deletes_a_user_stored_save_through_the_guarded_operation() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let source = directory.path().join("Vwings.dat");
        create_save(&source);
        let captured = repository
            .capture(
                &source,
                StoredSaveKind::Preset,
                "Practice".into(),
                None,
                StoredSaveOrigin::Current,
            )
            .unwrap();

        delete_stored_save(&repository, &captured.metadata.id).unwrap();

        assert!(repository.list().unwrap().is_empty());
    }

    #[test]
    fn unfinished_transaction_blocks_deletion() {
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

        let result = delete_stored_save(&repository, &captured.metadata.id);

        assert!(matches!(
            result,
            Err(StoredSaveDeleteError::RecoveryRequired(paths)) if paths == vec![journal_path]
        ));
        assert_eq!(1, repository.list().unwrap().len());
    }
}
