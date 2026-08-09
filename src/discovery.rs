use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::first_activation::FirstActivationError;
#[cfg(not(test))]
use crate::first_activation::suggested_current_filename;
use crate::known_folders::{self, KnownFolderError};

const SAVE_DIRECTORY_COMPONENTS: [&str; 4] = ["EA Games", "Mirror's Edge", "TdGame", "Savefiles"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentSave {
    path: PathBuf,
}

impl CurrentSave {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn filename(&self) -> &OsStr {
        self.path
            .file_name()
            .expect("a discovered save always has a filename")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentSaveDiscovery {
    SaveDirectoryMissing { directory: PathBuf },
    CurrentMissing { directory: PathBuf },
    CurrentFound(CurrentSave),
}

#[derive(Debug)]
pub enum DiscoveryError {
    KnownFolder(KnownFolderError),
    CurrentFilename(FirstActivationError),
    ReadDirectory {
        directory: PathBuf,
        source: io::Error,
    },
    InvalidSaveDirectory(PathBuf),
    InspectCurrent {
        path: PathBuf,
        source: io::Error,
    },
    InvalidCurrentType(PathBuf),
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KnownFolder(source) => write!(formatter, "failed to locate Documents: {source}"),
            Self::CurrentFilename(source) => {
                write!(
                    formatter,
                    "failed to determine the Current filename: {source}"
                )
            }
            Self::ReadDirectory { directory, source } => {
                write!(
                    formatter,
                    "failed to read {}: {source}",
                    directory.display()
                )
            }
            Self::InvalidSaveDirectory(path) => {
                write!(
                    formatter,
                    "save directory path is not a directory: {}",
                    path.display()
                )
            }
            Self::InspectCurrent { path, source } => write!(
                formatter,
                "failed to inspect Current at {}: {source}",
                path.display()
            ),
            Self::InvalidCurrentType(path) => {
                write!(
                    formatter,
                    "Current path is not a regular file: {}",
                    path.display()
                )
            }
        }
    }
}

impl Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadDirectory { source, .. } | Self::InspectCurrent { source, .. } => {
                Some(source)
            }
            Self::KnownFolder(source) => Some(source),
            Self::CurrentFilename(source) => Some(source),
            Self::InvalidSaveDirectory(_) | Self::InvalidCurrentType(_) => None,
        }
    }
}

impl From<KnownFolderError> for DiscoveryError {
    fn from(source: KnownFolderError) -> Self {
        Self::KnownFolder(source)
    }
}

pub fn discover_current() -> Result<CurrentSaveDiscovery, DiscoveryError> {
    discover_current_in_documents(&known_folders::documents()?)
}

pub fn discover_current_in_documents(
    documents_directory: &Path,
) -> Result<CurrentSaveDiscovery, DiscoveryError> {
    let save_directory = save_directory_in(documents_directory);

    let directory_metadata = match fs::metadata(&save_directory) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(CurrentSaveDiscovery::SaveDirectoryMissing {
                directory: save_directory,
            });
        }
        Err(source) => {
            return Err(DiscoveryError::ReadDirectory {
                directory: save_directory.clone(),
                source,
            });
        }
    };
    if !directory_metadata.is_dir() {
        return Err(DiscoveryError::InvalidSaveDirectory(save_directory));
    }

    let current_path = current_path_in_documents(documents_directory)?;
    match fs::symlink_metadata(&current_path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            Ok(CurrentSaveDiscovery::CurrentFound(CurrentSave {
                path: current_path,
            }))
        }
        Ok(_) => Err(DiscoveryError::InvalidCurrentType(current_path)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            Ok(CurrentSaveDiscovery::CurrentMissing {
                directory: save_directory,
            })
        }
        Err(source) => Err(DiscoveryError::InspectCurrent {
            path: current_path,
            source,
        }),
    }
}

pub fn save_directory_in(documents_directory: &Path) -> PathBuf {
    let mut path = documents_directory.to_path_buf();
    for component in SAVE_DIRECTORY_COMPONENTS {
        path.push(component);
    }
    path
}

pub(crate) fn current_path_in_documents(
    documents_directory: &Path,
) -> Result<PathBuf, DiscoveryError> {
    Ok(save_directory_in(documents_directory).join(current_filename()?))
}

#[cfg(test)]
fn current_filename() -> Result<String, DiscoveryError> {
    Ok("Vwings.dat".into())
}

#[cfg(not(test))]
fn current_filename() -> Result<String, DiscoveryError> {
    suggested_current_filename().map_err(DiscoveryError::CurrentFilename)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn create_save_directory(documents: &Path) -> PathBuf {
        let directory = save_directory_in(documents);
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn reports_a_missing_save_directory_without_creating_it() {
        let documents = TempDir::new().unwrap();
        let expected_directory = save_directory_in(documents.path());

        let result = discover_current_in_documents(documents.path()).unwrap();

        assert_eq!(
            CurrentSaveDiscovery::SaveDirectoryMissing {
                directory: expected_directory.clone()
            },
            result
        );
        assert!(!expected_directory.exists());
    }

    #[test]
    fn reports_an_existing_directory_without_a_current_save() {
        let documents = TempDir::new().unwrap();
        let directory = create_save_directory(documents.path());
        fs::write(directory.join("notes.txt"), b"not a save").unwrap();
        fs::create_dir(directory.join("folder.dat")).unwrap();
        fs::write(directory.join("history.dat"), b"backup").unwrap();

        let result = discover_current_in_documents(documents.path()).unwrap();

        assert_eq!(
            CurrentSaveDiscovery::CurrentMissing {
                directory: directory.clone()
            },
            result
        );
    }

    #[test]
    fn finds_the_account_named_current_case_insensitively() {
        let documents = TempDir::new().unwrap();
        let directory = create_save_directory(documents.path());
        let save = directory.join("Vwings.DAT");
        fs::write(&save, b"save bytes").unwrap();

        let result = discover_current_in_documents(documents.path()).unwrap();

        assert_eq!(
            CurrentSaveDiscovery::CurrentFound(CurrentSave {
                path: directory.join("Vwings.dat")
            }),
            result
        );
    }

    #[test]
    fn ignores_other_dat_files_when_account_named_current_exists() {
        let documents = TempDir::new().unwrap();
        let directory = create_save_directory(documents.path());
        let backup = directory.join("Alice.dat");
        let current = directory.join("Vwings.dat");
        fs::write(&current, b"current").unwrap();
        fs::write(&backup, b"backup").unwrap();

        let result = discover_current_in_documents(documents.path()).unwrap();

        assert_eq!(
            CurrentSaveDiscovery::CurrentFound(CurrentSave { path: current }),
            result
        );
    }

    #[test]
    fn reports_current_missing_when_only_backup_and_transaction_files_exist() {
        let documents = TempDir::new().unwrap();
        let directory = create_save_directory(documents.path());
        fs::write(
            directory.join(".mirrors-edge-save-manager-backup.replacement.dat"),
            b"staging",
        )
        .unwrap();
        fs::write(directory.join("old-run.dat"), b"backup").unwrap();

        let result = discover_current_in_documents(documents.path()).unwrap();

        assert_eq!(CurrentSaveDiscovery::CurrentMissing { directory }, result);
    }

    #[test]
    fn current_save_exposes_its_path_and_filename() {
        let save = CurrentSave {
            path: PathBuf::from(r"C:\Saves\Vwings.dat"),
        };

        assert_eq!(Path::new(r"C:\Saves\Vwings.dat"), save.path());
        assert_eq!(OsStr::new("Vwings.dat"), save.filename());
    }
}
