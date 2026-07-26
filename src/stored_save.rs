use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::save_file::SaveFingerprint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredSaveKind {
    Preset,
    Stash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredSaveOrigin {
    BuiltIn,
    Current,
    Imported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSaveMetadata {
    pub id: String,
    pub kind: StoredSaveKind,
    pub alias: String,
    pub description: Option<String>,
    pub origin: StoredSaveOrigin,
    pub created_at: SystemTime,
    pub source_filename: String,
    pub source_modified_at: Option<SystemTime>,
    pub fingerprint: SaveFingerprint,
}

impl StoredSaveMetadata {
    pub fn promote_to_preset(&mut self) {
        self.kind = StoredSaveKind::Preset;
    }

    pub fn source_filename(&self) -> &str {
        &self.source_filename
    }
}

#[cfg(test)]
mod tests {
    use crate::save_file::SaveHash;

    use super::*;

    #[test]
    fn promotion_changes_only_the_stored_save_classification() {
        let created_at = SystemTime::UNIX_EPOCH;
        let fingerprint = SaveFingerprint {
            size: 9_134_256,
            sha256: SaveHash::from_bytes([7; 32]),
        };
        let mut metadata = StoredSaveMetadata {
            id: "stored-save-1".into(),
            kind: StoredSaveKind::Stash,
            alias: "Before practice".into(),
            description: Some("Recovery point".into()),
            origin: StoredSaveOrigin::Current,
            created_at,
            source_filename: "Vwings.dat".into(),
            source_modified_at: Some(created_at),
            fingerprint,
        };

        metadata.promote_to_preset();

        assert_eq!(StoredSaveKind::Preset, metadata.kind);
        assert_eq!("Before practice", metadata.alias);
        assert_eq!(fingerprint, metadata.fingerprint);
        assert_eq!("Vwings.dat", metadata.source_filename());
    }
}
