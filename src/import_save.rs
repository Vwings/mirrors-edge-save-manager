use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::alias::{AliasError, resolve_import_alias};
use crate::mutation_guard::{MutationGuard, MutationGuardError};
use crate::recovery::{RecoveryError, unfinished_journals};
use crate::storage::{CaptureResult, StorageError, StoredSaveRepository};
use crate::stored_save::{StoredSaveKind, StoredSaveOrigin};

pub struct ImportSaveRequest {
    pub source: PathBuf,
    pub alias: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug)]
pub enum ImportSaveError {
    Alias(AliasError),
    MutationGuard(MutationGuardError),
    Recovery(RecoveryError),
    RecoveryRequired(Vec<PathBuf>),
    InvalidExtension(PathBuf),
    Storage(StorageError),
}

impl fmt::Display for ImportSaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alias(source) => write!(formatter, "invalid alias: {source}"),
            Self::MutationGuard(source) => write!(formatter, "import is blocked: {source}"),
            Self::Recovery(source) => write!(formatter, "transaction scan failed: {source}"),
            Self::RecoveryRequired(paths) => write!(
                formatter,
                "unfinished transaction recovery is required before import: {paths:?}"
            ),
            Self::InvalidExtension(path) => write!(
                formatter,
                "import source must have a .dat extension: {}",
                path.display()
            ),
            Self::Storage(source) => write!(formatter, "save import failed: {source}"),
        }
    }
}

impl Error for ImportSaveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Alias(source) => Some(source),
            Self::MutationGuard(source) => Some(source),
            Self::Recovery(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::RecoveryRequired(_) | Self::InvalidExtension(_) => None,
        }
    }
}

pub fn import_save(
    repository: &StoredSaveRepository,
    request: ImportSaveRequest,
) -> Result<CaptureResult, ImportSaveError> {
    let _guard = MutationGuard::acquire().map_err(ImportSaveError::MutationGuard)?;
    let unfinished = unfinished_journals(repository.root()).map_err(ImportSaveError::Recovery)?;
    if !unfinished.is_empty() {
        return Err(ImportSaveError::RecoveryRequired(unfinished));
    }
    require_dat_extension(&request.source)?;
    let alias =
        resolve_import_alias(request.alias, &request.source).map_err(ImportSaveError::Alias)?;
    repository
        .capture(
            &request.source,
            StoredSaveKind::Preset,
            alias,
            request.description,
            StoredSaveOrigin::Imported,
        )
        .map_err(ImportSaveError::Storage)
}

fn require_dat_extension(path: &Path) -> Result<(), ImportSaveError> {
    if path
        .extension()
        .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("dat"))
    {
        Ok(())
    } else {
        Err(ImportSaveError::InvalidExtension(path.to_path_buf()))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use std::fs::{self, File};
    use std::io::{Seek, SeekFrom, Write};

    use tempfile::TempDir;
    use uuid::Uuid;

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
    fn imports_a_verified_external_save_as_a_preset() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let source = directory.path().join("practice.DAT");
        create_save(&source, 1);
        let fingerprint = validate_and_fingerprint(&source).unwrap();

        let imported = import_save(
            &repository,
            ImportSaveRequest {
                source: source.clone(),
                alias: None,
                description: Some("External save".into()),
            },
        )
        .unwrap();

        assert_eq!(StoredSaveKind::Preset, imported.metadata.kind);
        assert_eq!("practice", imported.metadata.alias);
        assert_eq!(StoredSaveOrigin::Imported, imported.metadata.origin);
        assert_eq!("practice.DAT", imported.metadata.source_filename);
        assert_eq!(fingerprint, imported.metadata.fingerprint);
        assert_eq!(
            fingerprint,
            repository.verify(&imported.metadata.id).unwrap()
        );
    }

    #[test]
    fn reports_duplicate_content_but_keeps_each_import() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let source = directory.path().join("practice.dat");
        create_save(&source, 1);
        let first = import_save(
            &repository,
            ImportSaveRequest {
                source: source.clone(),
                alias: Some("First".into()),
                description: None,
            },
        )
        .unwrap();

        let second = import_save(
            &repository,
            ImportSaveRequest {
                source,
                alias: Some("Second".into()),
                description: None,
            },
        )
        .unwrap();

        assert_eq!(vec![first.metadata.id], second.duplicate_ids);
        assert_eq!(2, repository.list().unwrap().len());
    }

    #[test]
    fn rejects_non_dat_and_invalid_files_without_committing() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let wrong_extension = directory.path().join("save.bin");
        create_save(&wrong_extension, 1);
        let invalid = directory.path().join("small.dat");
        fs::write(&invalid, b"invalid").unwrap();

        let extension_result = import_save(
            &repository,
            ImportSaveRequest {
                source: wrong_extension,
                alias: Some("Wrong".into()),
                description: None,
            },
        );
        let invalid_result = import_save(
            &repository,
            ImportSaveRequest {
                source: invalid,
                alias: Some("Invalid".into()),
                description: None,
            },
        );

        assert!(matches!(
            extension_result,
            Err(ImportSaveError::InvalidExtension(_))
        ));
        assert!(matches!(invalid_result, Err(ImportSaveError::Storage(_))));
        assert!(repository.list().unwrap().is_empty());
    }

    #[test]
    fn unfinished_transaction_blocks_import() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let source = directory.path().join("practice.dat");
        create_save(&source, 1);
        let transactions = repository.root().join(TRANSACTIONS_DIRECTORY_NAME);
        fs::create_dir_all(&transactions).unwrap();
        let journal_path = transactions.join(format!("{}.json", Uuid::new_v4()));
        fs::write(&journal_path, b"unfinished").unwrap();

        let result = import_save(
            &repository,
            ImportSaveRequest {
                source,
                alias: Some("Blocked".into()),
                description: None,
            },
        );

        assert!(matches!(
            result,
            Err(ImportSaveError::RecoveryRequired(paths)) if paths == vec![journal_path]
        ));
        assert!(repository.list().unwrap().is_empty());
    }
}
