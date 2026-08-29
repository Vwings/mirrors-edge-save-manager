use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::bufread::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::alias::{AliasError, validate_alias};
use crate::built_in::BUILT_IN_RESOURCES;
use crate::known_folders::{self, KnownFolderError};
use crate::save_file::{
    SAVE_FILE_SIZE, SaveFileError, SaveFingerprint, SaveHash, fingerprint_reader,
    validate_and_fingerprint,
};
use crate::stored_save::{StoredSaveKind, StoredSaveMetadata, StoredSaveOrigin};
use crate::windows_file;

pub const APPLICATION_DIRECTORY_NAME: &str = "Mirror's Edge Save Manager";
pub const METADATA_SCHEMA_VERSION: u32 = 1;

const STORED_SAVES_DIRECTORY_NAME: &str = "stored-saves";
const METADATA_FILE_NAME: &str = "metadata.json";
const PAYLOAD_FILE_NAME: &str = "payload.dat.gz";
const SETTINGS_FILE_NAME: &str = "settings.json";
const SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureResult {
    pub metadata: StoredSaveMetadata,
    pub duplicate_ids: Vec<String>,
}

#[derive(Debug)]
pub enum StorageError {
    Alias(AliasError),
    Source(SaveFileError),
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    InvalidMetadata {
        path: PathBuf,
        reason: String,
    },
    PayloadMismatch {
        path: PathBuf,
        expected: SaveFingerprint,
        actual: SaveFingerprint,
    },
    UnknownBuiltIn(String),
    BuiltInImmutable(String),
    NotAStash(String),
    InvalidTimestamp,
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alias(source) => write!(formatter, "invalid alias: {source}"),
            Self::Source(source) => write!(formatter, "invalid source save: {source}"),
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} {}: {source}",
                path.display()
            ),
            Self::Json { path, source } => {
                write!(
                    formatter,
                    "invalid metadata at {}: {source}",
                    path.display()
                )
            }
            Self::InvalidMetadata { path, reason } => {
                write!(
                    formatter,
                    "invalid metadata at {}: {reason}",
                    path.display()
                )
            }
            Self::PayloadMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "payload at {} does not match metadata: expected {} bytes/{}, got {} bytes/{}",
                path.display(),
                expected.size,
                expected.sha256,
                actual.size,
                actual.sha256
            ),
            Self::UnknownBuiltIn(id) => write!(formatter, "unknown built-in Preset {id}"),
            Self::BuiltInImmutable(id) => {
                write!(formatter, "built-in Preset {id} is read-only")
            }
            Self::NotAStash(id) => write!(formatter, "StoredSave {id} is not a Stash"),
            Self::InvalidTimestamp => {
                formatter.write_str("a save timestamp is earlier than the Unix epoch")
            }
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Alias(source) => Some(source),
            Self::Source(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::InvalidMetadata { .. }
            | Self::PayloadMismatch { .. }
            | Self::UnknownBuiltIn(_)
            | Self::BuiltInImmutable(_)
            | Self::NotAStash(_)
            | Self::InvalidTimestamp => None,
        }
    }
}

impl From<SaveFileError> for StorageError {
    fn from(source: SaveFileError) -> Self {
        Self::Source(source)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltInResource {
    pub(crate) id: &'static str,
    pub(crate) version: u32,
    pub(crate) alias: &'static str,
    pub(crate) description: Option<&'static str>,
    pub(crate) source_filename: &'static str,
    pub(crate) created_at_millis: u64,
    pub(crate) fingerprint: SaveFingerprint,
    pub(crate) compressed_payload: &'static [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSaveRepository {
    root: PathBuf,
    built_ins: &'static [BuiltInResource],
}

impl StoredSaveRepository {
    pub fn for_current_user() -> Result<Self, KnownFolderError> {
        Ok(Self::with_built_ins(
            known_folders::local_app_data()?.join(APPLICATION_DIRECTORY_NAME),
            BUILT_IN_RESOURCES,
        ))
    }

    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            built_ins: &[],
        }
    }

    pub(crate) fn with_built_ins(root: PathBuf, built_ins: &'static [BuiltInResource]) -> Self {
        Self { root, built_ins }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn preferred_language(&self) -> Result<Option<String>, StorageError> {
        Ok(self.read_settings()?.language)
    }

    pub fn set_preferred_language(&self, language: Option<String>) -> Result<(), StorageError> {
        let mut settings = self.read_settings()?;
        settings.language = language;
        self.write_settings(&settings)
    }

    pub fn capture(
        &self,
        source: &Path,
        kind: StoredSaveKind,
        alias: String,
        description: Option<String>,
        origin: StoredSaveOrigin,
    ) -> Result<CaptureResult, StorageError> {
        let alias = validate_alias(alias).map_err(StorageError::Alias)?;
        let fingerprint = validate_and_fingerprint(source)?;
        let source_metadata = fs::metadata(source).map_err(|source_error| StorageError::Io {
            operation: "inspect source",
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let source_modified_at = source_metadata
            .modified()
            .ok()
            .map(truncate_to_millis)
            .transpose()?;
        let duplicate_ids = self
            .list()?
            .into_iter()
            .filter(|existing| existing.fingerprint == fingerprint)
            .map(|existing| existing.id)
            .collect();
        let id = Uuid::new_v4().to_string();
        let saves_directory = self.stored_saves_directory();
        create_directory_all(&saves_directory)?;

        let staging_directory = saves_directory.join(format!(".{id}.tmp"));
        let final_directory = saves_directory.join(&id);
        create_directory(&staging_directory)?;

        let result = (|| {
            let payload_path = staging_directory.join(PAYLOAD_FILE_NAME);
            compress_file(source, &payload_path)?;
            verify_payload(&payload_path, fingerprint)?;

            let metadata = StoredSaveMetadata {
                id,
                kind,
                alias,
                description,
                origin,
                created_at: truncate_to_millis(SystemTime::now())?,
                source_filename: source
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                source_modified_at,
                fingerprint,
            };
            write_metadata(&staging_directory.join(METADATA_FILE_NAME), &metadata)?;
            Ok(metadata)
        })();

        let metadata = match result {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = fs::remove_dir_all(&staging_directory);
                return Err(error);
            }
        };

        if let Err(source) = fs::rename(&staging_directory, &final_directory) {
            let _ = fs::remove_dir_all(&staging_directory);
            return Err(StorageError::Io {
                operation: "commit stored save",
                path: final_directory,
                source,
            });
        }

        Ok(CaptureResult {
            metadata,
            duplicate_ids,
        })
    }

    pub fn list(&self) -> Result<Vec<StoredSaveMetadata>, StorageError> {
        let saves_directory = self.stored_saves_directory();
        if !saves_directory
            .try_exists()
            .map_err(|source| StorageError::Io {
                operation: "inspect stored saves directory",
                path: saves_directory.clone(),
                source,
            })?
        {
            return self.visible_built_ins();
        }

        let entries = fs::read_dir(&saves_directory).map_err(|source| StorageError::Io {
            operation: "read stored saves directory",
            path: saves_directory.clone(),
            source,
        })?;
        let mut saves = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|source| StorageError::Io {
                operation: "read stored save entry",
                path: saves_directory.clone(),
                source,
            })?;
            let file_type = entry.file_type().map_err(|source| StorageError::Io {
                operation: "inspect stored save entry",
                path: entry.path(),
                source,
            })?;
            let name = entry.file_name();

            if !file_type.is_dir() || name.to_string_lossy().starts_with('.') {
                continue;
            }

            let metadata_path = entry.path().join(METADATA_FILE_NAME);
            let metadata = read_metadata(&metadata_path)?;
            if metadata.id != name.to_string_lossy() {
                return Err(StorageError::InvalidMetadata {
                    path: metadata_path,
                    reason: "stored save ID does not match its directory".into(),
                });
            }
            saves.push(metadata);
        }

        saves.extend(self.visible_built_ins()?);

        saves.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(saves)
    }

    pub fn verify(&self, id: &str) -> Result<SaveFingerprint, StorageError> {
        if let Some(resource) = self.built_in(id) {
            verify_embedded_payload(resource)?;
            return Ok(resource.fingerprint);
        }
        let (metadata, payload_path) = self.load_entry(id)?;
        verify_payload(&payload_path, metadata.fingerprint)?;
        Ok(metadata.fingerprint)
    }

    pub fn promote_to_preset(&self, id: &str) -> Result<StoredSaveMetadata, StorageError> {
        self.update_metadata(id, |metadata| {
            if metadata.kind != StoredSaveKind::Stash {
                return Err(StorageError::NotAStash(metadata.id.clone()));
            }
            metadata.promote_to_preset();
            Ok(())
        })
    }

    pub fn update_details(
        &self,
        id: &str,
        alias: String,
        description: Option<String>,
    ) -> Result<StoredSaveMetadata, StorageError> {
        self.update_metadata(id, |metadata| {
            metadata.alias = validate_alias(alias).map_err(StorageError::Alias)?;
            metadata.description = description;
            Ok(())
        })
    }

    pub fn delete(&self, id: &str) -> Result<(), StorageError> {
        let (_metadata, payload_path) = self.load_entry(id)?;
        let directory = payload_path
            .parent()
            .expect("a stored payload always has a parent");
        let tombstone = self
            .stored_saves_directory()
            .join(format!(".{id}.deleted.{}", Uuid::new_v4()));
        fs::rename(directory, &tombstone).map_err(|source| StorageError::Io {
            operation: "delete stored save",
            path: directory.to_path_buf(),
            source,
        })?;
        let _ = fs::remove_dir_all(tombstone);
        Ok(())
    }

    pub(crate) fn materialize_payload(
        &self,
        id: &str,
        destination: &Path,
    ) -> Result<SaveFingerprint, StorageError> {
        if let Some(resource) = self.built_in(id) {
            decompress_payload_reader(
                Cursor::new(resource.compressed_payload),
                destination,
                resource.fingerprint,
            )?;
            return Ok(resource.fingerprint);
        }
        let (metadata, payload_path) = self.load_entry(id)?;
        decompress_payload(&payload_path, destination, metadata.fingerprint)?;
        Ok(metadata.fingerprint)
    }

    fn load_entry(&self, id: &str) -> Result<(StoredSaveMetadata, PathBuf), StorageError> {
        if self.built_in(id).is_some() {
            return Err(StorageError::BuiltInImmutable(id.into()));
        }
        Uuid::parse_str(id).map_err(|_| StorageError::InvalidMetadata {
            path: self.stored_saves_directory().join(id),
            reason: "stored save ID is not a UUID".into(),
        })?;
        let directory = self.stored_saves_directory().join(id);
        let metadata = read_metadata(&directory.join(METADATA_FILE_NAME))?;
        if metadata.id != id {
            return Err(StorageError::InvalidMetadata {
                path: directory.join(METADATA_FILE_NAME),
                reason: "stored save ID does not match its directory".into(),
            });
        }
        Ok((metadata, directory.join(PAYLOAD_FILE_NAME)))
    }

    fn update_metadata(
        &self,
        id: &str,
        update: impl FnOnce(&mut StoredSaveMetadata) -> Result<(), StorageError>,
    ) -> Result<StoredSaveMetadata, StorageError> {
        let (mut metadata, payload_path) = self.load_entry(id)?;
        verify_payload(&payload_path, metadata.fingerprint)?;
        update(&mut metadata)?;

        let directory = payload_path
            .parent()
            .expect("a stored payload always has a parent");
        let metadata_path = directory.join(METADATA_FILE_NAME);
        let temporary_path = directory.join(format!(".{}.metadata.json.next", Uuid::new_v4()));
        write_metadata(&temporary_path, &metadata)?;
        if let Err(source) = windows_file::atomic_move(&temporary_path, &metadata_path, true) {
            let _ = fs::remove_file(&temporary_path);
            return Err(StorageError::Io {
                operation: "publish metadata update",
                path: metadata_path,
                source,
            });
        }
        Ok(metadata)
    }

    pub(crate) fn set_built_in_hidden(&self, id: &str, hidden: bool) -> Result<(), StorageError> {
        if self.built_in(id).is_none() {
            return Err(StorageError::UnknownBuiltIn(id.into()));
        }
        let mut settings = self.read_settings()?;
        let changed = if hidden {
            settings.hidden_built_in_ids.insert(id.into())
        } else {
            settings.hidden_built_in_ids.remove(id)
        };
        if changed {
            self.write_settings(&settings)?;
        }
        Ok(())
    }

    pub fn built_in_version(&self, id: &str) -> Option<u32> {
        self.built_in(id).map(|resource| resource.version)
    }

    fn visible_built_ins(&self) -> Result<Vec<StoredSaveMetadata>, StorageError> {
        let hidden = self.read_settings()?.hidden_built_in_ids;
        self.built_ins
            .iter()
            .filter(|resource| !hidden.contains(resource.id))
            .map(built_in_metadata)
            .collect()
    }

    fn built_in(&self, id: &str) -> Option<&BuiltInResource> {
        self.built_ins.iter().find(|resource| resource.id == id)
    }

    fn read_settings(&self) -> Result<SettingsDocument, StorageError> {
        let path = self.root.join(SETTINGS_FILE_NAME);
        if !path.try_exists().map_err(|source| StorageError::Io {
            operation: "inspect settings",
            path: path.clone(),
            source,
        })? {
            return Ok(SettingsDocument::default());
        }
        let file = File::open(&path).map_err(|source| StorageError::Io {
            operation: "open settings",
            path: path.clone(),
            source,
        })?;
        let settings: SettingsDocument =
            serde_json::from_reader(BufReader::new(file)).map_err(|source| StorageError::Json {
                path: path.clone(),
                source,
            })?;
        if settings.schema_version != SETTINGS_SCHEMA_VERSION {
            return Err(StorageError::InvalidMetadata {
                path,
                reason: format!("unsupported settings schema {}", settings.schema_version),
            });
        }
        if let Some(id) = settings
            .hidden_built_in_ids
            .iter()
            .find(|id| Uuid::parse_str(id).is_err())
        {
            return Err(StorageError::InvalidMetadata {
                path,
                reason: format!("hidden built-in ID is not a UUID: {id}"),
            });
        }
        Ok(settings)
    }

    fn write_settings(&self, settings: &SettingsDocument) -> Result<(), StorageError> {
        create_directory_all(&self.root)?;
        let path = self.root.join(SETTINGS_FILE_NAME);
        let temporary_path = self
            .root
            .join(format!(".{}.settings.json.next", Uuid::new_v4()));
        let serialized =
            serde_json::to_vec_pretty(settings).map_err(|source| StorageError::Json {
                path: temporary_path.clone(),
                source,
            })?;
        let mut file = File::create(&temporary_path).map_err(|source| StorageError::Io {
            operation: "create settings",
            path: temporary_path.clone(),
            source,
        })?;
        file.write_all(&serialized)
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|source| StorageError::Io {
                operation: "write settings",
                path: temporary_path.clone(),
                source,
            })?;
        if let Err(source) = windows_file::atomic_move(&temporary_path, &path, true) {
            let _ = fs::remove_file(&temporary_path);
            return Err(StorageError::Io {
                operation: "publish settings",
                path,
                source,
            });
        }
        Ok(())
    }

    fn stored_saves_directory(&self) -> PathBuf {
        self.root.join(STORED_SAVES_DIRECTORY_NAME)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SettingsDocument {
    schema_version: u32,
    hidden_built_in_ids: BTreeSet<String>,
    #[serde(default)]
    language: Option<String>,
}

impl Default for SettingsDocument {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            hidden_built_in_ids: BTreeSet::new(),
            language: None,
        }
    }
}

fn built_in_metadata(resource: &BuiltInResource) -> Result<StoredSaveMetadata, StorageError> {
    let path = embedded_resource_path(resource.id);
    if Uuid::parse_str(resource.id).is_err() {
        return Err(StorageError::InvalidMetadata {
            path,
            reason: "built-in ID is not a UUID".into(),
        });
    }
    if resource.version == 0 || resource.fingerprint.size != SAVE_FILE_SIZE {
        return Err(StorageError::InvalidMetadata {
            path,
            reason: "built-in version and size must be valid".into(),
        });
    }
    let alias = validate_alias(resource.alias.into()).map_err(StorageError::Alias)?;
    Ok(StoredSaveMetadata {
        id: resource.id.into(),
        kind: StoredSaveKind::Preset,
        alias,
        description: resource.description.map(Into::into),
        origin: StoredSaveOrigin::BuiltIn,
        created_at: UNIX_EPOCH + Duration::from_millis(resource.created_at_millis),
        source_filename: resource.source_filename.into(),
        source_modified_at: None,
        fingerprint: resource.fingerprint,
    })
}

fn embedded_resource_path(id: &str) -> PathBuf {
    PathBuf::from(format!("embedded-built-in-{id}.dat.gz"))
}

#[derive(Debug, Serialize, Deserialize)]
struct MetadataDocument {
    schema_version: u32,
    id: String,
    kind: StoredSaveKind,
    alias: String,
    description: Option<String>,
    origin: StoredSaveOrigin,
    created_at_unix_millis: u64,
    source_filename: String,
    source_modified_at_unix_millis: Option<u64>,
    original_size: u64,
    sha256: String,
    compression: String,
}

impl MetadataDocument {
    fn from_metadata(metadata: &StoredSaveMetadata) -> Result<Self, StorageError> {
        Ok(Self {
            schema_version: METADATA_SCHEMA_VERSION,
            id: metadata.id.clone(),
            kind: metadata.kind,
            alias: metadata.alias.clone(),
            description: metadata.description.clone(),
            origin: metadata.origin,
            created_at_unix_millis: time_to_millis(metadata.created_at)?,
            source_filename: metadata.source_filename.clone(),
            source_modified_at_unix_millis: metadata
                .source_modified_at
                .map(time_to_millis)
                .transpose()?,
            original_size: metadata.fingerprint.size,
            sha256: metadata.fingerprint.sha256.to_string(),
            compression: "gzip".into(),
        })
    }

    fn into_metadata(self, path: &Path) -> Result<StoredSaveMetadata, StorageError> {
        if self.schema_version != METADATA_SCHEMA_VERSION {
            return Err(StorageError::InvalidMetadata {
                path: path.to_path_buf(),
                reason: format!("unsupported schema version {}", self.schema_version),
            });
        }
        if self.compression != "gzip" {
            return Err(StorageError::InvalidMetadata {
                path: path.to_path_buf(),
                reason: format!("unsupported compression {}", self.compression),
            });
        }
        if self.original_size != SAVE_FILE_SIZE {
            return Err(StorageError::InvalidMetadata {
                path: path.to_path_buf(),
                reason: format!("unexpected original size {}", self.original_size),
            });
        }
        if Uuid::parse_str(&self.id).is_err() {
            return Err(StorageError::InvalidMetadata {
                path: path.to_path_buf(),
                reason: "stored save ID is not a UUID".into(),
            });
        }

        let sha256 =
            self.sha256
                .parse::<SaveHash>()
                .map_err(|source| StorageError::InvalidMetadata {
                    path: path.to_path_buf(),
                    reason: source.to_string(),
                })?;
        let created_at = millis_to_time(self.created_at_unix_millis, path)?;
        let source_modified_at = self
            .source_modified_at_unix_millis
            .map(|millis| millis_to_time(millis, path))
            .transpose()?;

        Ok(StoredSaveMetadata {
            id: self.id,
            kind: self.kind,
            alias: self.alias,
            description: self.description,
            origin: self.origin,
            created_at,
            source_filename: self.source_filename,
            source_modified_at,
            fingerprint: SaveFingerprint {
                size: self.original_size,
                sha256,
            },
        })
    }
}

fn write_metadata(path: &Path, metadata: &StoredSaveMetadata) -> Result<(), StorageError> {
    let document = MetadataDocument::from_metadata(metadata)?;
    let serialized = serde_json::to_vec_pretty(&document).map_err(|source| StorageError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    let mut file = File::create(path).map_err(|source| StorageError::Io {
        operation: "create metadata",
        path: path.to_path_buf(),
        source,
    })?;
    file.write_all(&serialized)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|source| StorageError::Io {
            operation: "write metadata",
            path: path.to_path_buf(),
            source,
        })
}

fn read_metadata(path: &Path) -> Result<StoredSaveMetadata, StorageError> {
    let file = File::open(path).map_err(|source| StorageError::Io {
        operation: "open metadata",
        path: path.to_path_buf(),
        source,
    })?;
    let document =
        serde_json::from_reader::<_, MetadataDocument>(BufReader::new(file)).map_err(|source| {
            StorageError::Json {
                path: path.to_path_buf(),
                source,
            }
        })?;
    document.into_metadata(path)
}

fn compress_file(source: &Path, destination: &Path) -> Result<(), StorageError> {
    let input = File::open(source).map_err(|source_error| StorageError::Io {
        operation: "open source for compression",
        path: source.to_path_buf(),
        source: source_error,
    })?;
    let output = File::create(destination).map_err(|source| StorageError::Io {
        operation: "create compressed payload",
        path: destination.to_path_buf(),
        source,
    })?;
    let mut encoder = GzEncoder::new(BufWriter::new(output), Compression::best());
    io::copy(&mut BufReader::new(input), &mut encoder).map_err(|source| StorageError::Io {
        operation: "compress source save",
        path: destination.to_path_buf(),
        source,
    })?;
    let mut output = encoder.finish().map_err(|source| StorageError::Io {
        operation: "finish compressed payload",
        path: destination.to_path_buf(),
        source,
    })?;
    output.flush().map_err(|source| StorageError::Io {
        operation: "flush compressed payload",
        path: destination.to_path_buf(),
        source,
    })?;
    output
        .get_ref()
        .sync_all()
        .map_err(|source| StorageError::Io {
            operation: "sync compressed payload",
            path: destination.to_path_buf(),
            source,
        })
}

fn verify_payload(path: &Path, expected: SaveFingerprint) -> Result<(), StorageError> {
    let file = File::open(path).map_err(|source| StorageError::Io {
        operation: "open compressed payload",
        path: path.to_path_buf(),
        source,
    })?;
    let decoder = GzDecoder::new(BufReader::new(file));
    let actual = fingerprint_reader(decoder.take(SAVE_FILE_SIZE + 1)).map_err(|source| {
        StorageError::Io {
            operation: "decompress payload",
            path: path.to_path_buf(),
            source,
        }
    })?;

    if actual != expected {
        return Err(StorageError::PayloadMismatch {
            path: path.to_path_buf(),
            expected,
            actual,
        });
    }
    Ok(())
}

fn verify_embedded_payload(resource: &BuiltInResource) -> Result<(), StorageError> {
    let path = embedded_resource_path(resource.id);
    let decoder = GzDecoder::new(Cursor::new(resource.compressed_payload));
    let actual = fingerprint_reader(decoder.take(SAVE_FILE_SIZE + 1)).map_err(|source| {
        StorageError::Io {
            operation: "decompress embedded payload",
            path: path.clone(),
            source,
        }
    })?;
    if actual != resource.fingerprint {
        return Err(StorageError::PayloadMismatch {
            path,
            expected: resource.fingerprint,
            actual,
        });
    }
    Ok(())
}

fn decompress_payload(
    source: &Path,
    destination: &Path,
    expected: SaveFingerprint,
) -> Result<(), StorageError> {
    let input = File::open(source).map_err(|source_error| StorageError::Io {
        operation: "open compressed payload",
        path: source.to_path_buf(),
        source: source_error,
    })?;
    decompress_payload_reader(BufReader::new(input), destination, expected)
}

fn decompress_payload_reader(
    input: impl BufRead,
    destination: &Path,
    expected: SaveFingerprint,
) -> Result<(), StorageError> {
    let output = File::options()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|source| StorageError::Io {
            operation: "create staged payload",
            path: destination.to_path_buf(),
            source,
        })?;
    let result = {
        let decoder = GzDecoder::new(input);
        let mut output = BufWriter::new(output);

        (|| {
            io::copy(&mut decoder.take(SAVE_FILE_SIZE + 1), &mut output).map_err(|source| {
                StorageError::Io {
                    operation: "decompress staged payload",
                    path: destination.to_path_buf(),
                    source,
                }
            })?;
            output.flush().map_err(|source| StorageError::Io {
                operation: "flush staged payload",
                path: destination.to_path_buf(),
                source,
            })?;
            output
                .get_ref()
                .sync_all()
                .map_err(|source| StorageError::Io {
                    operation: "sync staged payload",
                    path: destination.to_path_buf(),
                    source,
                })?;

            let file = File::open(destination).map_err(|source| StorageError::Io {
                operation: "open staged payload for verification",
                path: destination.to_path_buf(),
                source,
            })?;
            let actual = fingerprint_reader(BufReader::new(file).take(SAVE_FILE_SIZE + 1))
                .map_err(|source| StorageError::Io {
                    operation: "verify staged payload",
                    path: destination.to_path_buf(),
                    source,
                })?;

            if actual != expected {
                return Err(StorageError::PayloadMismatch {
                    path: destination.to_path_buf(),
                    expected,
                    actual,
                });
            }
            Ok(())
        })()
    };

    if let Err(error) = result {
        if let Err(source) = fs::remove_file(destination) {
            return Err(StorageError::Io {
                operation: "remove failed staged payload",
                path: destination.to_path_buf(),
                source,
            });
        }
        return Err(error);
    }
    Ok(())
}

fn create_directory_all(path: &Path) -> Result<(), StorageError> {
    fs::create_dir_all(path).map_err(|source| StorageError::Io {
        operation: "create directory",
        path: path.to_path_buf(),
        source,
    })
}

fn create_directory(path: &Path) -> Result<(), StorageError> {
    fs::create_dir(path).map_err(|source| StorageError::Io {
        operation: "create staging directory",
        path: path.to_path_buf(),
        source,
    })
}

fn truncate_to_millis(time: SystemTime) -> Result<SystemTime, StorageError> {
    Ok(UNIX_EPOCH + Duration::from_millis(time_to_millis(time)?))
}

fn time_to_millis(time: SystemTime) -> Result<u64, StorageError> {
    let millis = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StorageError::InvalidTimestamp)?
        .as_millis();
    u64::try_from(millis).map_err(|_| StorageError::InvalidTimestamp)
}

fn millis_to_time(millis: u64, path: &Path) -> Result<SystemTime, StorageError> {
    UNIX_EPOCH
        .checked_add(Duration::from_millis(millis))
        .ok_or_else(|| StorageError::InvalidMetadata {
            path: path.to_path_buf(),
            reason: "timestamp is out of range".into(),
        })
}

#[cfg(test)]
mod tests {
    use std::io::{Seek, SeekFrom};

    use tempfile::TempDir;

    use super::*;

    fn create_valid_save(directory: &Path, name: &str) -> PathBuf {
        let path = directory.join(name);
        let mut file = File::create(&path).unwrap();
        file.set_len(SAVE_FILE_SIZE).unwrap();
        file.seek(SeekFrom::Start(SAVE_FILE_SIZE - 1)).unwrap();
        file.write_all(&[1]).unwrap();
        file.flush().unwrap();
        path
    }

    #[test]
    fn captures_lists_and_verifies_a_stored_save() {
        let directory = TempDir::new().unwrap();
        let source = create_valid_save(directory.path(), "Vwings.dat");
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));

        let captured = repository
            .capture(
                &source,
                StoredSaveKind::Stash,
                "Before practice".into(),
                Some("Automatic recovery point".into()),
                StoredSaveOrigin::Current,
            )
            .unwrap();

        assert!(captured.duplicate_ids.is_empty());
        assert_eq!("Vwings.dat", captured.metadata.source_filename);
        assert_eq!(
            captured.metadata.fingerprint,
            repository.verify(&captured.metadata.id).unwrap()
        );

        let listed = repository.list().unwrap();
        assert_eq!(vec![captured.metadata.clone()], listed);

        let payload = repository
            .root()
            .join(STORED_SAVES_DIRECTORY_NAME)
            .join(&captured.metadata.id)
            .join(PAYLOAD_FILE_NAME);
        assert!(fs::metadata(payload).unwrap().len() < SAVE_FILE_SIZE);
    }

    #[test]
    fn reports_duplicate_content_but_keeps_both_entries() {
        let directory = TempDir::new().unwrap();
        let source = create_valid_save(directory.path(), "Vwings.dat");
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let first = repository
            .capture(
                &source,
                StoredSaveKind::Preset,
                "First".into(),
                None,
                StoredSaveOrigin::Imported,
            )
            .unwrap();

        let second = repository
            .capture(
                &source,
                StoredSaveKind::Preset,
                "Second".into(),
                None,
                StoredSaveOrigin::Imported,
            )
            .unwrap();

        assert_eq!(vec![first.metadata.id], second.duplicate_ids);
        assert_eq!(2, repository.list().unwrap().len());
    }

    #[test]
    fn invalid_source_does_not_create_a_stored_save() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("invalid.dat");
        fs::write(&source, b"invalid").unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));

        let result = repository.capture(
            &source,
            StoredSaveKind::Stash,
            "Invalid".into(),
            None,
            StoredSaveOrigin::Imported,
        );

        assert!(matches!(result, Err(StorageError::Source(_))));
        assert!(repository.list().unwrap().is_empty());
    }

    #[test]
    fn detects_a_corrupted_compressed_payload() {
        let directory = TempDir::new().unwrap();
        let source = create_valid_save(directory.path(), "Vwings.dat");
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let captured = repository
            .capture(
                &source,
                StoredSaveKind::Stash,
                "Recovery".into(),
                None,
                StoredSaveOrigin::Current,
            )
            .unwrap();
        let payload = repository
            .stored_saves_directory()
            .join(&captured.metadata.id)
            .join(PAYLOAD_FILE_NAME);
        fs::write(payload, b"not gzip").unwrap();

        let result = repository.verify(&captured.metadata.id);

        assert!(matches!(result, Err(StorageError::Io { .. })));
    }

    #[test]
    fn reports_a_storage_root_that_is_not_a_directory() {
        let directory = TempDir::new().unwrap();
        let source = create_valid_save(directory.path(), "Vwings.dat");
        let root = directory.path().join("app-data");
        fs::write(&root, b"this blocks directory creation").unwrap();
        let repository = StoredSaveRepository::new(root);

        let result = repository.capture(
            &source,
            StoredSaveKind::Stash,
            "Recovery".into(),
            None,
            StoredSaveOrigin::Current,
        );

        assert!(matches!(result, Err(StorageError::Io { .. })));
    }

    #[test]
    fn persists_preferred_language_without_affecting_hidden_built_ins() {
        let directory = TempDir::new().unwrap();
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));

        assert_eq!(None, repository.preferred_language().unwrap());
        repository
            .set_preferred_language(Some("zh-CN".into()))
            .unwrap();

        let reopened = StoredSaveRepository::new(directory.path().join("app-data"));
        assert_eq!(Some("zh-CN".into()), reopened.preferred_language().unwrap());
    }

    #[test]
    fn promotes_and_edits_metadata_without_rewriting_payload() {
        let directory = TempDir::new().unwrap();
        let source = create_valid_save(directory.path(), "Vwings.dat");
        let repository = StoredSaveRepository::new(directory.path().join("app-data"));
        let captured = repository
            .capture(
                &source,
                StoredSaveKind::Stash,
                "Recovery".into(),
                None,
                StoredSaveOrigin::Current,
            )
            .unwrap();
        let payload = repository
            .stored_saves_directory()
            .join(&captured.metadata.id)
            .join(PAYLOAD_FILE_NAME);
        let original_payload = fs::read(&payload).unwrap();

        let promoted = repository.promote_to_preset(&captured.metadata.id).unwrap();
        let edited = repository
            .update_details(
                &captured.metadata.id,
                "Practice start".into(),
                Some("Chapter practice".into()),
            )
            .unwrap();

        assert_eq!(StoredSaveKind::Preset, promoted.kind);
        assert_eq!(StoredSaveKind::Preset, edited.kind);
        assert_eq!("Practice start", edited.alias);
        assert_eq!(Some("Chapter practice"), edited.description.as_deref());
        assert_eq!(captured.metadata.fingerprint, edited.fingerprint);
        assert_eq!(original_payload, fs::read(payload).unwrap());
        assert_eq!(vec![edited], repository.list().unwrap());
    }
}
