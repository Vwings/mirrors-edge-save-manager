use std::error::Error;
use std::fmt;
use std::path::Path;

pub const MAX_ALIAS_CHARACTERS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AliasError {
    Empty,
    TooLong { characters: usize },
}

impl fmt::Display for AliasError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("alias must not be empty"),
            Self::TooLong { characters } => write!(
                formatter,
                "alias has {characters} characters, maximum is {MAX_ALIAS_CHARACTERS}"
            ),
        }
    }
}

impl Error for AliasError {}

pub fn validate_alias(alias: String) -> Result<String, AliasError> {
    let alias = alias.trim();
    if alias.is_empty() {
        return Err(AliasError::Empty);
    }
    let characters = alias.chars().count();
    if characters > MAX_ALIAS_CHARACTERS {
        return Err(AliasError::TooLong { characters });
    }
    Ok(alias.into())
}

pub(crate) fn resolve_current_alias(
    alias: Option<String>,
    classification: &str,
) -> Result<String, AliasError> {
    resolve_alias(alias, || timestamped_alias(classification))
}

pub(crate) fn resolve_import_alias(
    alias: Option<String>,
    source: &Path,
) -> Result<String, AliasError> {
    resolve_alias(alias, || {
        source
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .and_then(|stem| validate_alias(stem).ok())
            .unwrap_or_else(|| timestamped_alias("Preset"))
    })
}

fn resolve_alias(
    alias: Option<String>,
    default: impl FnOnce() -> String,
) -> Result<String, AliasError> {
    validate_alias(alias.unwrap_or_else(default))
}

#[cfg(windows)]
fn timestamped_alias(classification: &str) -> String {
    use std::mem::MaybeUninit;

    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    let mut time = MaybeUninit::<SYSTEMTIME>::uninit();
    unsafe { GetLocalTime(time.as_mut_ptr()) };
    let time = unsafe { time.assume_init() };
    format!(
        "{classification} {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
    )
}

#[cfg(not(windows))]
fn timestamped_alias(classification: &str) -> String {
    classification.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_validates_aliases_by_unicode_character_count() {
        assert_eq!("Practice", validate_alias("  Practice  ".into()).unwrap());
        assert!(matches!(
            validate_alias("   ".into()),
            Err(AliasError::Empty)
        ));
        assert!(validate_alias("界".repeat(MAX_ALIAS_CHARACTERS)).is_ok());
        assert!(matches!(
            validate_alias("界".repeat(MAX_ALIAS_CHARACTERS + 1)),
            Err(AliasError::TooLong { .. })
        ));
    }

    #[test]
    fn uses_filename_stem_for_import_defaults() {
        assert_eq!(
            "practice",
            resolve_import_alias(None, Path::new("C:/saves/practice.dat")).unwrap()
        );
    }

    #[test]
    fn generates_timestamped_current_defaults() {
        let alias = resolve_current_alias(None, "Stash").unwrap();

        assert!(alias.starts_with("Stash "));
        assert!(alias.len() > "Stash ".len());
    }
}
