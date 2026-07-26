use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownFolderError {
    Windows(i32),
    EmptyPath,
    UnsupportedPlatform,
}

impl fmt::Display for KnownFolderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Windows(result) => write!(
                formatter,
                "failed to resolve a Windows known folder (HRESULT 0x{result:08X})"
            ),
            Self::EmptyPath => formatter.write_str("Windows returned an empty known folder path"),
            Self::UnsupportedPlatform => {
                formatter.write_str("known folder discovery is only supported on Windows")
            }
        }
    }
}

impl Error for KnownFolderError {}

#[cfg(windows)]
pub fn documents() -> Result<PathBuf, KnownFolderError> {
    use windows_sys::Win32::UI::Shell::FOLDERID_Documents;

    resolve(&FOLDERID_Documents)
}

#[cfg(not(windows))]
pub fn documents() -> Result<PathBuf, KnownFolderError> {
    Err(KnownFolderError::UnsupportedPlatform)
}

#[cfg(windows)]
pub fn local_app_data() -> Result<PathBuf, KnownFolderError> {
    use windows_sys::Win32::UI::Shell::FOLDERID_LocalAppData;

    resolve(&FOLDERID_LocalAppData)
}

#[cfg(not(windows))]
pub fn local_app_data() -> Result<PathBuf, KnownFolderError> {
    Err(KnownFolderError::UnsupportedPlatform)
}

#[cfg(windows)]
fn resolve(folder_id: &windows_sys::core::GUID) -> Result<PathBuf, KnownFolderError> {
    use std::ffi::{OsString, c_void};
    use std::os::windows::ffi::OsStringExt;
    use std::ptr;

    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{KF_FLAG_DEFAULT, SHGetKnownFolderPath};

    let mut raw_path = ptr::null_mut();
    let result = unsafe {
        SHGetKnownFolderPath(
            folder_id,
            KF_FLAG_DEFAULT as u32,
            ptr::null_mut(),
            &mut raw_path,
        )
    };

    if result < 0 {
        if !raw_path.is_null() {
            unsafe {
                CoTaskMemFree(raw_path.cast::<c_void>());
            }
        }
        return Err(KnownFolderError::Windows(result));
    }

    if raw_path.is_null() {
        return Err(KnownFolderError::EmptyPath);
    }

    let length = unsafe {
        let mut length = 0;
        while *raw_path.add(length) != 0 {
            length += 1;
        }
        length
    };
    let path = PathBuf::from(OsString::from_wide(unsafe {
        std::slice::from_raw_parts(raw_path, length)
    }));

    unsafe {
        CoTaskMemFree(raw_path.cast::<c_void>());
    }

    if path.as_os_str().is_empty() {
        Err(KnownFolderError::EmptyPath)
    } else {
        Ok(path)
    }
}
