use std::error::Error;
use std::fmt;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::save_file::SaveFingerprint;

pub const MAX_DESCRIPTION_CHARACTERS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptionError {
    TooLong { characters: usize },
}

impl fmt::Display for DescriptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { characters } => write!(
                formatter,
                "description has {characters} characters, maximum is {MAX_DESCRIPTION_CHARACTERS}"
            ),
        }
    }
}

impl Error for DescriptionError {}

pub fn validate_description(
    description: Option<String>,
) -> Result<Option<String>, DescriptionError> {
    let Some(description) = description else {
        return Ok(None);
    };
    let description = description.trim();
    if description.is_empty() {
        return Ok(None);
    }
    let characters = description.graphemes(true).count();
    if characters > MAX_DESCRIPTION_CHARACTERS {
        return Err(DescriptionError::TooLong { characters });
    }
    Ok(Some(description.into()))
}

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
pub struct AppliedSourceSnapshot {
    pub stored_save_id: String,
    pub kind: StoredSaveKind,
    pub origin: StoredSaveOrigin,
    pub alias: String,
    pub applied_at: SystemTime,
    pub fingerprint: SaveFingerprint,
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
    pub capture_source: Option<AppliedSourceSnapshot>,
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
            capture_source: None,
        };

        metadata.promote_to_preset();

        assert_eq!(StoredSaveKind::Preset, metadata.kind);
        assert_eq!("Before practice", metadata.alias);
        assert_eq!(fingerprint, metadata.fingerprint);
        assert_eq!("Vwings.dat", metadata.source_filename());
    }

    #[test]
    fn trims_and_validates_descriptions_by_unicode_character_count() {
        assert_eq!(
            Some("Practice notes".into()),
            validate_description(Some("  Practice notes  ".into())).unwrap()
        );
        assert_eq!(None, validate_description(Some("   ".into())).unwrap());
        assert!(validate_description(Some("界".repeat(MAX_DESCRIPTION_CHARACTERS))).is_ok());
        assert!(matches!(
            validate_description(Some("界".repeat(MAX_DESCRIPTION_CHARACTERS + 1))),
            Err(DescriptionError::TooLong { .. })
        ));
    }

    #[test]
    fn counts_user_perceived_description_characters_like_the_ui() {
        let emoji = "👩‍💻".repeat(MAX_DESCRIPTION_CHARACTERS);
        assert!(validate_description(Some(emoji)).is_ok());

        let combining_character = "e\u{301}".repeat(MAX_DESCRIPTION_CHARACTERS + 1);
        assert!(matches!(
            validate_description(Some(combining_character)),
            Err(DescriptionError::TooLong { characters }) if characters == MAX_DESCRIPTION_CHARACTERS + 1
        ));
    }
}
