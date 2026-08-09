use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::save_file::SaveFingerprint;
use crate::windows_file;

pub(crate) const TRANSACTIONS_DIRECTORY_NAME: &str = "transactions";
pub(crate) const JOURNAL_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApplyPhase {
    Prepared,
    Replacing,
    Replaced,
    Verified,
    RollingBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActivationPhase {
    Prepared,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ApplyJournal {
    pub(crate) schema_version: u32,
    pub(crate) transaction_id: String,
    pub(crate) operation: String,
    pub(crate) phase: ApplyPhase,
    created_at_unix_millis: u64,
    updated_at_unix_millis: u64,
    pub(crate) stored_save_id: String,
    pub(crate) automatic_stash_id: String,
    pub(crate) current_path: PathBuf,
    pub(crate) replacement_path: PathBuf,
    pub(crate) rollback_path: PathBuf,
    pub(crate) failed_replacement_path: PathBuf,
    pub(crate) original_fingerprint: JournalFingerprint,
    pub(crate) replacement_fingerprint: JournalFingerprint,
}

impl ApplyJournal {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        transaction_id: String,
        stored_save_id: String,
        automatic_stash_id: String,
        current_path: PathBuf,
        replacement_path: PathBuf,
        rollback_path: PathBuf,
        failed_replacement_path: PathBuf,
        original_fingerprint: SaveFingerprint,
        replacement_fingerprint: SaveFingerprint,
    ) -> io::Result<Self> {
        let now = unix_millis()?;
        Ok(Self {
            schema_version: JOURNAL_SCHEMA_VERSION,
            transaction_id,
            operation: "apply".into(),
            phase: ApplyPhase::Prepared,
            created_at_unix_millis: now,
            updated_at_unix_millis: now,
            stored_save_id,
            automatic_stash_id,
            current_path,
            replacement_path,
            rollback_path,
            failed_replacement_path,
            original_fingerprint: original_fingerprint.into(),
            replacement_fingerprint: replacement_fingerprint.into(),
        })
    }

    pub(crate) fn set_phase(&mut self, phase: ApplyPhase) -> io::Result<()> {
        self.phase = phase;
        self.updated_at_unix_millis = unix_millis()?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ActivationJournal {
    pub(crate) schema_version: u32,
    pub(crate) transaction_id: String,
    pub(crate) operation: String,
    pub(crate) phase: ActivationPhase,
    created_at_unix_millis: u64,
    updated_at_unix_millis: u64,
    pub(crate) stored_save_id: String,
    pub(crate) current_path: PathBuf,
    pub(crate) staging_path: PathBuf,
    pub(crate) replacement_fingerprint: JournalFingerprint,
}

impl ActivationJournal {
    pub(crate) fn new(
        transaction_id: String,
        stored_save_id: String,
        current_path: PathBuf,
        staging_path: PathBuf,
        replacement_fingerprint: SaveFingerprint,
    ) -> io::Result<Self> {
        let now = unix_millis()?;
        Ok(Self {
            schema_version: JOURNAL_SCHEMA_VERSION,
            transaction_id,
            operation: "activate".into(),
            phase: ActivationPhase::Prepared,
            created_at_unix_millis: now,
            updated_at_unix_millis: now,
            stored_save_id,
            current_path,
            staging_path,
            replacement_fingerprint: replacement_fingerprint.into(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct JournalFingerprint {
    pub(crate) size: u64,
    pub(crate) sha256: String,
}

impl From<SaveFingerprint> for JournalFingerprint {
    fn from(value: SaveFingerprint) -> Self {
        Self {
            size: value.size,
            sha256: value.sha256.to_string(),
        }
    }
}

impl TryFrom<&JournalFingerprint> for SaveFingerprint {
    type Error = crate::save_file::ParseSaveHashError;

    fn try_from(value: &JournalFingerprint) -> Result<Self, Self::Error> {
        Ok(Self {
            size: value.size,
            sha256: value.sha256.parse()?,
        })
    }
}

pub(crate) struct JournalStore {
    path: PathBuf,
    temporary_path: PathBuf,
    published: bool,
}

impl JournalStore {
    pub(crate) fn new(application_root: &Path, transaction_id: &str) -> Self {
        let directory = application_root.join(TRANSACTIONS_DIRECTORY_NAME);
        Self {
            path: directory.join(format!("{transaction_id}.json")),
            temporary_path: directory.join(format!(".{transaction_id}.json.next")),
            published: false,
        }
    }

    pub(crate) fn publish(&mut self, journal: &impl Serialize) -> io::Result<()> {
        let directory = self
            .path
            .parent()
            .expect("a transaction journal always has a parent");
        fs::create_dir_all(directory)?;
        let serialized = serde_json::to_vec_pretty(journal).map_err(io::Error::other)?;
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(&self.temporary_path)?;
        file.write_all(&serialized)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);

        let result = windows_file::atomic_move(&self.temporary_path, &self.path, self.published);
        if result.is_ok() {
            self.published = true;
        }
        result
    }

    pub(crate) fn remove(self) -> io::Result<()> {
        fs::remove_file(self.path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

fn unix_millis() -> io::Result<u64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_millis();
    u64::try_from(millis).map_err(io::Error::other)
}
