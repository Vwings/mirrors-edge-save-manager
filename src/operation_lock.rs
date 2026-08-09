use std::error::Error;
use std::fmt;
use std::io;

const MUTATION_MUTEX_NAME: &str = "Local\\MirrorsEdgeSaveManager.Mutation.v1";

#[derive(Debug)]
pub enum OperationLockError {
    Create(io::Error),
    Wait(io::Error),
    UnexpectedWaitStatus(u32),
    UnsupportedPlatform,
}

impl fmt::Display for OperationLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create(source) => {
                write!(formatter, "failed to create the operation lock: {source}")
            }
            Self::Wait(source) => {
                write!(formatter, "failed to acquire the operation lock: {source}")
            }
            Self::UnexpectedWaitStatus(status) => {
                write!(
                    formatter,
                    "operation lock returned unexpected wait status 0x{status:08X}"
                )
            }
            Self::UnsupportedPlatform => {
                formatter.write_str("the operation lock is only supported on Windows")
            }
        }
    }
}

impl Error for OperationLockError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Create(source) | Self::Wait(source) => Some(source),
            Self::UnexpectedWaitStatus(_) | Self::UnsupportedPlatform => None,
        }
    }
}

#[must_use = "dropping the operation lock releases it immediately"]
pub struct OperationLock {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

impl OperationLock {
    #[cfg(windows)]
    pub fn try_acquire() -> Result<Option<Self>, OperationLockError> {
        Self::try_acquire_named(MUTATION_MUTEX_NAME)
    }

    #[cfg(not(windows))]
    pub fn try_acquire() -> Result<Option<Self>, OperationLockError> {
        Err(OperationLockError::UnsupportedPlatform)
    }

    #[cfg(windows)]
    fn try_acquire_named(name: &str) -> Result<Option<Self>, OperationLockError> {
        use std::ptr;

        use windows_sys::Win32::Foundation::{
            CloseHandle, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
        };
        use windows_sys::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};

        let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
        let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(OperationLockError::Create(io::Error::last_os_error()));
        }

        match unsafe { WaitForSingleObject(handle, 0) } {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Some(Self { handle })),
            WAIT_TIMEOUT => {
                unsafe {
                    CloseHandle(handle);
                }
                Ok(None)
            }
            WAIT_FAILED => {
                let source = io::Error::last_os_error();
                unsafe {
                    CloseHandle(handle);
                }
                Err(OperationLockError::Wait(source))
            }
            status => {
                unsafe {
                    CloseHandle(handle);
                }
                Err(OperationLockError::UnexpectedWaitStatus(status))
            }
        }
    }
}

#[cfg(windows)]
impl Drop for OperationLock {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::ReleaseMutex;

        unsafe {
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;

    use super::*;

    static NEXT_LOCK_ID: AtomicU64 = AtomicU64::new(0);

    fn unique_lock_name() -> String {
        format!(
            "Local\\MirrorsEdgeSaveManager.Test.{}.{}",
            std::process::id(),
            NEXT_LOCK_ID.fetch_add(1, Ordering::Relaxed)
        )
    }

    #[test]
    fn blocks_another_thread_until_the_lock_is_released() {
        let name = unique_lock_name();
        let first = OperationLock::try_acquire_named(&name).unwrap().unwrap();

        let blocked_name = name.clone();
        let blocked = thread::spawn(move || {
            OperationLock::try_acquire_named(&blocked_name)
                .unwrap()
                .is_none()
        })
        .join()
        .unwrap();
        assert!(blocked);

        drop(first);

        let acquired =
            thread::spawn(move || OperationLock::try_acquire_named(&name).unwrap().is_some())
                .join()
                .unwrap();
        assert!(acquired);
    }
}
