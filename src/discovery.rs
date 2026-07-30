use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use uuid::Uuid;

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
    SaveDirectoryMissing {
        directory: PathBuf,
    },
    CurrentMissing {
        directory: PathBuf,
    },
    CurrentFound(CurrentSave),
    CurrentAmbiguous {
        directory: PathBuf,
        candidates: Vec<PathBuf>,
    },
}

#[derive(Debug)]
pub enum DiscoveryError {
    KnownFolder(KnownFolderError),
    ReadDirectory {
        directory: PathBuf,
        source: io::Error,
    },
    InspectEntry {
        directory: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KnownFolder(source) => write!(formatter, "failed to locate Documents: {source}"),
            Self::ReadDirectory { directory, source } => {
                write!(
                    formatter,
                    "failed to read {}: {source}",
                    directory.display()
                )
            }
            Self::InspectEntry { directory, source } => write!(
                formatter,
                "failed to inspect an entry in {}: {source}",
                directory.display()
            ),
        }
    }
}

impl Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadDirectory { source, .. } | Self::InspectEntry { source, .. } => Some(source),
            Self::KnownFolder(source) => Some(source),
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

    if !save_directory
        .try_exists()
        .map_err(|source| DiscoveryError::ReadDirectory {
            directory: save_directory.clone(),
            source,
        })?
    {
        return Ok(CurrentSaveDiscovery::SaveDirectoryMissing {
            directory: save_directory,
        });
    }

    let entries =
        fs::read_dir(&save_directory).map_err(|source| DiscoveryError::ReadDirectory {
            directory: save_directory.clone(),
            source,
        })?;
    let mut candidates = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|source| DiscoveryError::InspectEntry {
            directory: save_directory.clone(),
            source,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source| DiscoveryError::InspectEntry {
                directory: save_directory.clone(),
                source,
            })?;

        if file_type.is_file()
            && has_dat_extension(&entry.path())
            && !is_transaction_artifact(&entry.file_name())
        {
            candidates.push(entry.path());
        }
    }

    candidates.sort();

    match candidates.len() {
        0 => Ok(CurrentSaveDiscovery::CurrentMissing {
            directory: save_directory,
        }),
        1 => Ok(CurrentSaveDiscovery::CurrentFound(CurrentSave {
            path: candidates.pop().expect("one candidate was found"),
        })),
        _ => Ok(CurrentSaveDiscovery::CurrentAmbiguous {
            directory: save_directory,
            candidates,
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

fn has_dat_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("dat"))
}

fn is_transaction_artifact(name: &OsStr) -> bool {
    const PREFIX: &str = ".mirrors-edge-save-switcher-";
    const SUFFIXES: [&str; 3] = [".replacement.dat", ".rollback.dat", ".failed.dat"];

    let name = name.to_string_lossy();
    let Some(remainder) = name.strip_prefix(PREFIX) else {
        return false;
    };
    SUFFIXES.iter().any(|suffix| {
        remainder
            .strip_suffix(suffix)
            .is_some_and(|id| Uuid::parse_str(id).is_ok())
    })
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

        let result = discover_current_in_documents(documents.path()).unwrap();

        assert_eq!(
            CurrentSaveDiscovery::CurrentMissing {
                directory: directory.clone()
            },
            result
        );
    }

    #[test]
    fn finds_one_dat_file_case_insensitively() {
        let documents = TempDir::new().unwrap();
        let directory = create_save_directory(documents.path());
        let save = directory.join("Vwings.DAT");
        fs::write(&save, b"save bytes").unwrap();

        let result = discover_current_in_documents(documents.path()).unwrap();

        assert_eq!(
            CurrentSaveDiscovery::CurrentFound(CurrentSave { path: save }),
            result
        );
    }

    #[test]
    fn reports_all_candidates_when_current_is_ambiguous() {
        let documents = TempDir::new().unwrap();
        let directory = create_save_directory(documents.path());
        let first = directory.join("Alice.dat");
        let second = directory.join("Vwings.dat");
        fs::write(&second, b"second").unwrap();
        fs::write(&first, b"first").unwrap();

        let result = discover_current_in_documents(documents.path()).unwrap();

        assert_eq!(
            CurrentSaveDiscovery::CurrentAmbiguous {
                directory,
                candidates: vec![first, second]
            },
            result
        );
    }

    #[test]
    fn ignores_owned_transaction_artifacts_but_not_similar_user_files() {
        let documents = TempDir::new().unwrap();
        let directory = create_save_directory(documents.path());
        let current = directory.join("Vwings.dat");
        fs::write(&current, b"current").unwrap();
        let id = Uuid::new_v4();
        fs::write(
            directory.join(format!(".mirrors-edge-save-switcher-{id}.replacement.dat")),
            b"staging",
        )
        .unwrap();
        let similar = directory.join(".mirrors-edge-save-switcher-not-a-uuid.rollback.dat");
        fs::write(&similar, b"user file").unwrap();

        let result = discover_current_in_documents(documents.path()).unwrap();

        assert_eq!(
            CurrentSaveDiscovery::CurrentAmbiguous {
                directory,
                candidates: vec![similar, current]
            },
            result
        );
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
