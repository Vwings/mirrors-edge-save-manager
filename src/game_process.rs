use std::error::Error;
use std::fmt;
use std::io;

const GAME_EXECUTABLE: &str = "MirrorsEdge.exe";

#[derive(Debug)]
pub enum GameProcessError {
    Snapshot(io::Error),
    Enumerate(io::Error),
    UnsupportedPlatform,
}

impl fmt::Display for GameProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Snapshot(source) => {
                write!(formatter, "failed to create a process snapshot: {source}")
            }
            Self::Enumerate(source) => {
                write!(formatter, "failed to enumerate running processes: {source}")
            }
            Self::UnsupportedPlatform => {
                formatter.write_str("game process detection is only supported on Windows")
            }
        }
    }
}

impl Error for GameProcessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Snapshot(source) | Self::Enumerate(source) => Some(source),
            Self::UnsupportedPlatform => None,
        }
    }
}

#[cfg(windows)]
pub fn is_game_running() -> Result<bool, GameProcessError> {
    use std::mem;

    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(GameProcessError::Snapshot(io::Error::last_os_error()));
    }

    struct Snapshot(windows_sys::Win32::Foundation::HANDLE);

    impl Drop for Snapshot {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    let snapshot = Snapshot(snapshot);
    let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
    entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;

    if unsafe { Process32FirstW(snapshot.0, &mut entry) } == 0 {
        let source = io::Error::last_os_error();
        return if source.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
            Ok(false)
        } else {
            Err(GameProcessError::Enumerate(source))
        };
    }

    loop {
        if executable_name_matches(&entry.szExeFile) {
            return Ok(true);
        }

        if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
            let source = io::Error::last_os_error();
            return if source.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                Ok(false)
            } else {
                Err(GameProcessError::Enumerate(source))
            };
        }
    }
}

#[cfg(not(windows))]
pub fn is_game_running() -> Result<bool, GameProcessError> {
    Err(GameProcessError::UnsupportedPlatform)
}

fn executable_name_matches(name: &[u16]) -> bool {
    let length = name
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(name.len());
    String::from_utf16_lossy(&name[..length]).eq_ignore_ascii_case(GAME_EXECUTABLE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide_name(name: &str) -> Vec<u16> {
        name.encode_utf16().chain([0]).collect()
    }

    #[test]
    fn matches_the_game_executable_case_insensitively() {
        assert!(executable_name_matches(&wide_name("MirrorsEdge.exe")));
        assert!(executable_name_matches(&wide_name("mirrorsedge.EXE")));
    }

    #[test]
    fn rejects_other_or_partial_executable_names() {
        assert!(!executable_name_matches(&wide_name("MirrorsEdge.exe.old")));
        assert!(!executable_name_matches(&wide_name("MirrorsEdge")));
        assert!(!executable_name_matches(&wide_name("OtherGame.exe")));
    }

    #[test]
    fn ignores_data_after_the_first_null_terminator() {
        let mut name = wide_name("MirrorsEdge.exe");
        name.extend("ignored".encode_utf16());

        assert!(executable_name_matches(&name));
    }
}
