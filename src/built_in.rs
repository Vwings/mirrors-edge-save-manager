use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use crate::mutation_guard::{MutationGuard, MutationGuardError};
use crate::recovery::{RecoveryError, unfinished_journals};
use crate::save_file::{SAVE_FILE_SIZE, SaveFingerprint, SaveHash};
use crate::storage::{BuiltInResource, StorageError, StoredSaveRepository};

pub const COMPLETED_CAMPAIGN_ID: &str = "06802ae2-c71c-4164-bc6b-76ed1f194955";
pub const SPEEDRUN_69_ID: &str = "fedd92be-af26-4a72-8c89-9059b9043b20";
pub const ALL_TIME_TRIALS_UNLOCKED_ID: &str = "8283c9ab-83fc-4f40-8ac5-3a37d7a0200d";
pub const NEW_GAME_ID: &str = "0bbc9387-8ebb-4eab-829b-f799e1e5a7b3";

pub(crate) const NEW_GAME: BuiltInResource = BuiltInResource {
    id: NEW_GAME_ID,
    version: 1,
    alias: "New Game",
    description: Some("Fresh save before starting the campaign"),
    source_filename: "fresh-before.dat",
    created_at_millis: 1_785_715_200_000,
    fingerprint: SaveFingerprint {
        size: SAVE_FILE_SIZE,
        sha256: SaveHash::from_bytes([
            0xc5, 0x1a, 0xbd, 0x1e, 0x24, 0x83, 0xac, 0x07, 0x53, 0x67, 0x76, 0xca, 0xe4, 0x40,
            0x52, 0x25, 0xa7, 0xcb, 0x57, 0xee, 0x3a, 0x04, 0x93, 0x10, 0x36, 0x18, 0xdd, 0x02,
            0x99, 0x24, 0x59, 0x5b,
        ]),
    },
    compressed_payload: include_bytes!("../resources/built-in/new-game-v1.dat.gz"),
};

pub(crate) const COMPLETED_CAMPAIGN: BuiltInResource = BuiltInResource {
    id: COMPLETED_CAMPAIGN_ID,
    version: 1,
    alias: "Completed Campaign",
    description: Some("Completed campaign starting save"),
    source_filename: "game_finished_blank.dat",
    created_at_millis: 1_785_715_200_000,
    fingerprint: SaveFingerprint {
        size: SAVE_FILE_SIZE,
        sha256: SaveHash::from_bytes([
            0x86, 0x4c, 0x27, 0xb9, 0xed, 0xc0, 0xe2, 0x10, 0x2c, 0xa1, 0xa5, 0x39, 0x63, 0x47,
            0x5a, 0x54, 0x2d, 0x16, 0xc6, 0xbf, 0x61, 0x5a, 0x9e, 0x49, 0x8b, 0x15, 0x0c, 0xe4,
            0xf0, 0xfd, 0xf0, 0xf5,
        ]),
    },
    compressed_payload: include_bytes!("../resources/built-in/completed-campaign-v1.dat.gz"),
};

pub(crate) const SPEEDRUN_69: BuiltInResource = BuiltInResource {
    id: SPEEDRUN_69_ID,
    version: 1,
    alias: "69% Speedrun",
    description: Some("Community 69% speedrun starting save"),
    source_filename: "69.dat",
    created_at_millis: 1_785_715_200_000,
    fingerprint: SaveFingerprint {
        size: SAVE_FILE_SIZE,
        sha256: SaveHash::from_bytes([
            0xa5, 0xac, 0xe2, 0xce, 0xf4, 0x2f, 0x38, 0xc2, 0xe1, 0xbd, 0x42, 0x16, 0x8f, 0xbb,
            0x7d, 0xc1, 0x89, 0x82, 0xc0, 0xe5, 0xf6, 0xf0, 0x63, 0x23, 0xee, 0xe9, 0xd7, 0x26,
            0xde, 0x22, 0x8a, 0x49,
        ]),
    },
    compressed_payload: include_bytes!("../resources/built-in/speedrun-69-v1.dat.gz"),
};

pub(crate) const ALL_TIME_TRIALS_UNLOCKED: BuiltInResource = BuiltInResource {
    id: ALL_TIME_TRIALS_UNLOCKED_ID,
    version: 1,
    alias: "All Time Trials Unlocked",
    description: Some("Completed campaign with all time trials unlocked and no PB times or Ghosts"),
    source_filename: "completed-time-trials-unlocked.dat",
    created_at_millis: 1_785_715_200_000,
    fingerprint: SaveFingerprint {
        size: SAVE_FILE_SIZE,
        sha256: SaveHash::from_bytes([
            0x5c, 0xd0, 0x3a, 0x0c, 0x58, 0xe3, 0xda, 0x3c, 0x7c, 0xb9, 0xfa, 0xe1, 0x6c, 0xcc,
            0x56, 0x85, 0xd6, 0xb4, 0x81, 0x6f, 0xfb, 0x15, 0xf5, 0x75, 0x3a, 0x8c, 0xe7, 0x75,
            0x36, 0x08, 0x70, 0xda,
        ]),
    },
    compressed_payload: include_bytes!("../resources/built-in/all-time-trials-unlocked-v1.dat.gz"),
};

pub(crate) static BUILT_IN_RESOURCES: &[BuiltInResource] = &[
    NEW_GAME,
    COMPLETED_CAMPAIGN,
    SPEEDRUN_69,
    ALL_TIME_TRIALS_UNLOCKED,
];

#[derive(Debug)]
pub enum BuiltInPresetError {
    MutationGuard(MutationGuardError),
    Recovery(RecoveryError),
    RecoveryRequired(Vec<PathBuf>),
    Storage(StorageError),
}

impl fmt::Display for BuiltInPresetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MutationGuard(source) => {
                write!(formatter, "built-in Preset update is blocked: {source}")
            }
            Self::Recovery(source) => write!(formatter, "transaction scan failed: {source}"),
            Self::RecoveryRequired(paths) => write!(
                formatter,
                "unfinished transaction recovery is required before changing built-in Presets: {paths:?}"
            ),
            Self::Storage(source) => write!(formatter, "built-in Preset update failed: {source}"),
        }
    }
}

impl Error for BuiltInPresetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MutationGuard(source) => Some(source),
            Self::Recovery(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::RecoveryRequired(_) => None,
        }
    }
}

pub fn hide_built_in_preset(
    repository: &StoredSaveRepository,
    id: &str,
) -> Result<(), BuiltInPresetError> {
    set_hidden(repository, id, true)
}

pub fn restore_built_in_preset(
    repository: &StoredSaveRepository,
    id: &str,
) -> Result<(), BuiltInPresetError> {
    set_hidden(repository, id, false)
}

fn set_hidden(
    repository: &StoredSaveRepository,
    id: &str,
    hidden: bool,
) -> Result<(), BuiltInPresetError> {
    let _guard = MutationGuard::acquire().map_err(BuiltInPresetError::MutationGuard)?;
    let unfinished =
        unfinished_journals(repository.root()).map_err(BuiltInPresetError::Recovery)?;
    if !unfinished.is_empty() {
        return Err(BuiltInPresetError::RecoveryRequired(unfinished));
    }
    repository
        .set_built_in_hidden(id, hidden)
        .map_err(BuiltInPresetError::Storage)
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::{Seek, SeekFrom, Write};

    use tempfile::TempDir;

    use crate::apply::{ApplyRequest, apply_in_documents};
    use crate::save_file::validate_and_fingerprint;
    use crate::stored_save::StoredSaveOrigin;

    use super::*;

    #[test]
    fn embedded_resources_match_their_manifests() {
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::with_built_ins(
            directory.path().join("app-data"),
            BUILT_IN_RESOURCES,
        );

        let listed = repository.list().unwrap();
        assert_eq!(4, listed.len());
        assert!(
            listed
                .iter()
                .all(|save| save.origin == StoredSaveOrigin::BuiltIn)
        );
        for resource in BUILT_IN_RESOURCES {
            assert_eq!(
                resource.fingerprint,
                repository.verify(resource.id).unwrap()
            );
            assert_eq!(
                Some(resource.version),
                repository.built_in_version(resource.id)
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn applies_an_embedded_resource_without_consuming_it() {
        use crate::mutation_guard::MUTATION_GUARD_TEST;

        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let documents = directory.path().join("Documents");
        let save_directory = documents.join("EA Games/Mirror's Edge/TdGame/Savefiles");
        fs::create_dir_all(&save_directory).unwrap();
        let current = save_directory.join("Vwings.dat");
        let mut file = File::create(&current).unwrap();
        file.set_len(SAVE_FILE_SIZE).unwrap();
        file.seek(SeekFrom::Start(SAVE_FILE_SIZE - 1)).unwrap();
        file.write_all(&[1]).unwrap();
        file.sync_all().unwrap();

        let repository = StoredSaveRepository::with_built_ins(
            directory.path().join("app-data"),
            BUILT_IN_RESOURCES,
        );
        let result = apply_in_documents(
            &repository,
            &documents,
            ApplyRequest {
                stored_save_id: COMPLETED_CAMPAIGN_ID,
                automatic_stash_alias: Some("Before built-in".into()),
                automatic_stash_description: None,
            },
        )
        .unwrap();

        assert_eq!(COMPLETED_CAMPAIGN.fingerprint, result.applied_fingerprint);
        assert_eq!(
            COMPLETED_CAMPAIGN.fingerprint,
            validate_and_fingerprint(&current).unwrap()
        );
        assert_eq!(5, repository.list().unwrap().len());
        assert_eq!(
            COMPLETED_CAMPAIGN.fingerprint,
            repository.verify(COMPLETED_CAMPAIGN_ID).unwrap()
        );
    }

    #[cfg(windows)]
    #[test]
    fn hiding_survives_resource_upgrades_and_restoring_reveals_it() {
        use crate::mutation_guard::MUTATION_GUARD_TEST;

        static UPGRADED: &[BuiltInResource] = &[BuiltInResource {
            version: 2,
            ..COMPLETED_CAMPAIGN
        }];

        let _test = MUTATION_GUARD_TEST.lock().unwrap();
        let directory = TempDir::new().unwrap();
        let root = directory.path().join("app-data");
        let repository = StoredSaveRepository::with_built_ins(root.clone(), &[COMPLETED_CAMPAIGN]);

        hide_built_in_preset(&repository, COMPLETED_CAMPAIGN_ID).unwrap();
        assert!(repository.list().unwrap().is_empty());

        let temporarily_removed = StoredSaveRepository::new(root.clone());
        assert!(temporarily_removed.list().unwrap().is_empty());

        let upgraded = StoredSaveRepository::with_built_ins(root, UPGRADED);
        assert!(upgraded.list().unwrap().is_empty());
        restore_built_in_preset(&upgraded, COMPLETED_CAMPAIGN_ID).unwrap();
        assert_eq!(1, upgraded.list().unwrap().len());
        assert_eq!(Some(2), upgraded.built_in_version(COMPLETED_CAMPAIGN_ID));
    }
}
