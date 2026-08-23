use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum FileDialogError {
    Platform(u32),
    UnsupportedPlatform,
}

impl fmt::Display for FileDialogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Platform(code) => {
                write!(formatter, "Windows file dialog failed with code {code}")
            }
            Self::UnsupportedPlatform => {
                formatter.write_str("the save-file dialog requires Windows")
            }
        }
    }
}

impl Error for FileDialogError {}

#[cfg(windows)]
pub fn select_save_file() -> Result<Option<PathBuf>, FileDialogError> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    use windows_sys::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY,
        OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    const BUFFER_LENGTH: usize = 32_768;
    let mut file_buffer = vec![0u16; BUFFER_LENGTH];
    let filter = "Mirror's Edge saves (*.dat)\0*.dat\0All files (*.*)\0*.*\0\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let title = "Import a Mirror's Edge save"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let default_extension = "dat\0".encode_utf16().collect::<Vec<_>>();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: unsafe { GetForegroundWindow() },
        lpstrFilter: filter.as_ptr(),
        lpstrFile: file_buffer.as_mut_ptr(),
        nMaxFile: BUFFER_LENGTH as u32,
        lpstrTitle: title.as_ptr(),
        Flags: OFN_EXPLORER
            | OFN_FILEMUSTEXIST
            | OFN_PATHMUSTEXIST
            | OFN_HIDEREADONLY
            | OFN_NOCHANGEDIR,
        lpstrDefExt: default_extension.as_ptr(),
        ..Default::default()
    };

    if unsafe { GetOpenFileNameW(&mut dialog) } != 0 {
        let length = file_buffer
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(file_buffer.len());
        return Ok(Some(PathBuf::from(OsString::from_wide(
            &file_buffer[..length],
        ))));
    }

    let code = unsafe { CommDlgExtendedError() };
    if code == 0 {
        Ok(None)
    } else {
        Err(FileDialogError::Platform(code))
    }
}

#[cfg(not(windows))]
pub fn select_save_file() -> Result<Option<PathBuf>, FileDialogError> {
    Err(FileDialogError::UnsupportedPlatform)
}
