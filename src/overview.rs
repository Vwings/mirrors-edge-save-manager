use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::application_error::{ApplicationError, UserAction};
use crate::discovery::{CurrentSaveDiscovery, discover_current};
use crate::game_process::{GameProcessError, is_game_running};
use crate::recovery::recover_unfinished_transactions;
use crate::save_file::{SaveFingerprint, validate_and_fingerprint};
use crate::storage::StoredSaveRepository;
use crate::stored_save::{
    AppliedSourceSnapshot, StoredSaveKind, StoredSaveMetadata, StoredSaveOrigin,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverviewFailure {
    pub action: UserAction,
    pub detail: String,
}

impl From<ApplicationError> for OverviewFailure {
    fn from(error: ApplicationError) -> Self {
        Self {
            action: error.action(),
            detail: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameOverview {
    Available,
    Running,
    Unavailable(OverviewFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryOverview {
    Clear,
    Recovered(usize),
    BlockedByGame,
    Unavailable(OverviewFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentOverview {
    pub path: PathBuf,
    pub modified_at: Option<SystemTime>,
    pub fingerprint: SaveFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentSaveOverview {
    Found(CurrentOverview),
    Missing { directory: PathBuf },
    SaveDirectoryMissing { directory: PathBuf },
    Unavailable(OverviewFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSaveOverview {
    pub id: String,
    pub kind: StoredSaveKind,
    pub alias: String,
    pub description: Option<String>,
    pub origin: StoredSaveOrigin,
    pub created_at: SystemTime,
    pub source_filename: String,
    pub fingerprint: SaveFingerprint,
    pub capture_source: Option<AppliedSourceSnapshot>,
}

impl From<StoredSaveMetadata> for StoredSaveOverview {
    fn from(metadata: StoredSaveMetadata) -> Self {
        Self {
            id: metadata.id,
            kind: metadata.kind,
            alias: metadata.alias,
            description: metadata.description,
            origin: metadata.origin,
            created_at: metadata.created_at,
            source_filename: metadata.source_filename,
            fingerprint: metadata.fingerprint,
            capture_source: metadata.capture_source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSaveCollectionOverview {
    pub presets: Vec<StoredSaveOverview>,
    pub stashes: Vec<StoredSaveOverview>,
    pub failure: Option<OverviewFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationOverview {
    pub game: GameOverview,
    pub recovery: RecoveryOverview,
    pub current: CurrentSaveOverview,
    pub stored_saves: StoredSaveCollectionOverview,
    pub last_applied: Option<AppliedSourceSnapshot>,
}

pub fn load_application_overview() -> ApplicationOverview {
    let game = load_game_overview();
    let repository = StoredSaveRepository::for_current_user()
        .map_err(|error| OverviewFailure::from(ApplicationError::from(error)));

    let recovery = match (&game, &repository) {
        (GameOverview::Running, _) => RecoveryOverview::BlockedByGame,
        (GameOverview::Unavailable(failure), _) => RecoveryOverview::Unavailable(failure.clone()),
        (GameOverview::Available, Err(failure)) => RecoveryOverview::Unavailable(failure.clone()),
        (GameOverview::Available, Ok(repository)) => {
            match recover_unfinished_transactions(repository) {
                Ok(recovered) if recovered.is_empty() => RecoveryOverview::Clear,
                Ok(recovered) => RecoveryOverview::Recovered(recovered.len()),
                Err(error) => RecoveryOverview::Unavailable(ApplicationError::from(error).into()),
            }
        }
    };

    let current = load_current_overview();
    let last_applied = repository
        .as_ref()
        .ok()
        .and_then(|repository| repository.last_applied_source().ok().flatten());
    let stored_saves = load_stored_saves(repository);

    ApplicationOverview {
        game,
        recovery,
        current,
        stored_saves,
        last_applied,
    }
}

fn load_game_overview() -> GameOverview {
    match is_game_running() {
        Ok(true) => GameOverview::Running,
        Ok(false) => GameOverview::Available,
        Err(error) => GameOverview::Unavailable(OverviewFailure {
            action: match error {
                GameProcessError::UnsupportedPlatform => UserAction::UseSupportedPlatform,
                GameProcessError::Snapshot(_) | GameProcessError::Enumerate(_) => UserAction::Retry,
            },
            detail: error.to_string(),
        }),
    }
}

fn load_current_overview() -> CurrentSaveOverview {
    match discover_current() {
        Ok(CurrentSaveDiscovery::CurrentFound(current)) => {
            let path = current.path().to_path_buf();
            match validate_and_fingerprint(&path) {
                Ok(fingerprint) => CurrentSaveOverview::Found(CurrentOverview {
                    modified_at: fs::metadata(&path)
                        .and_then(|metadata| metadata.modified())
                        .ok(),
                    path,
                    fingerprint,
                }),
                Err(error) => CurrentSaveOverview::Unavailable(OverviewFailure {
                    action: UserAction::RefreshCurrent,
                    detail: error.to_string(),
                }),
            }
        }
        Ok(CurrentSaveDiscovery::CurrentMissing { directory }) => {
            CurrentSaveOverview::Missing { directory }
        }
        Ok(CurrentSaveDiscovery::SaveDirectoryMissing { directory }) => {
            CurrentSaveOverview::SaveDirectoryMissing { directory }
        }
        Err(error) => CurrentSaveOverview::Unavailable(ApplicationError::from(error).into()),
    }
}

fn load_stored_saves(
    repository: Result<StoredSaveRepository, OverviewFailure>,
) -> StoredSaveCollectionOverview {
    let saves = match repository {
        Ok(repository) => repository
            .list()
            .map_err(|error| OverviewFailure::from(ApplicationError::from(error))),
        Err(failure) => Err(failure),
    };

    match saves {
        Ok(saves) => {
            let (presets, stashes) = saves
                .into_iter()
                .map(StoredSaveOverview::from)
                .partition(|save| save.kind == StoredSaveKind::Preset);
            StoredSaveCollectionOverview {
                presets,
                stashes,
                failure: None,
            }
        }
        Err(failure) => StoredSaveCollectionOverview {
            presets: Vec::new(),
            stashes: Vec::new(),
            failure: Some(failure),
        },
    }
}
