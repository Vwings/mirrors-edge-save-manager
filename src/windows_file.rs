use std::io;
use std::path::Path;

#[cfg(windows)]
pub(crate) fn atomic_move(source: &Path, destination: &Path, replace: bool) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain([0]).collect();
    let destination: Vec<u16> = destination.as_os_str().encode_wide().chain([0]).collect();
    let mut flags = MOVEFILE_WRITE_THROUGH;
    if replace {
        flags |= MOVEFILE_REPLACE_EXISTING;
    }

    if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub(crate) fn atomic_move(_source: &Path, _destination: &Path, _replace: bool) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic file moves are only supported on Windows",
    ))
}

#[cfg(windows)]
pub(crate) fn replace_file(current: &Path, replacement: &Path, rollback: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    let current: Vec<u16> = current.as_os_str().encode_wide().chain([0]).collect();
    let replacement: Vec<u16> = replacement.as_os_str().encode_wide().chain([0]).collect();
    let rollback: Vec<u16> = rollback.as_os_str().encode_wide().chain([0]).collect();

    if unsafe {
        ReplaceFileW(
            current.as_ptr(),
            replacement.as_ptr(),
            rollback.as_ptr(),
            0,
            ptr::null(),
            ptr::null(),
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub(crate) fn replace_file(
    _current: &Path,
    _replacement: &Path,
    _rollback: &Path,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic file replacement is only supported on Windows",
    ))
}
