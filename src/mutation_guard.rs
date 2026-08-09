use std::error::Error;
use std::fmt;

#[cfg(not(test))]
use crate::game_process;
use crate::game_process::GameProcessError;
use crate::operation_lock::{OperationLock, OperationLockError};

#[cfg(test)]
pub(crate) static MUTATION_GUARD_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug)]
pub enum MutationGuardError {
    OperationInProgress,
    OperationLock(OperationLockError),
    GameProcess(GameProcessError),
    GameRunning,
}

impl fmt::Display for MutationGuardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OperationInProgress => {
                formatter.write_str("another save manager operation is already in progress")
            }
            Self::OperationLock(source) => write!(formatter, "operation lock failed: {source}"),
            Self::GameProcess(source) => write!(formatter, "game process check failed: {source}"),
            Self::GameRunning => {
                formatter.write_str("Mirror's Edge is running and blocks save mutations")
            }
        }
    }
}

impl Error for MutationGuardError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::OperationLock(source) => Some(source),
            Self::GameProcess(source) => Some(source),
            Self::OperationInProgress | Self::GameRunning => None,
        }
    }
}

impl From<OperationLockError> for MutationGuardError {
    fn from(source: OperationLockError) -> Self {
        Self::OperationLock(source)
    }
}

impl From<GameProcessError> for MutationGuardError {
    fn from(source: GameProcessError) -> Self {
        Self::GameProcess(source)
    }
}

#[must_use = "the mutation guard must remain alive for the full mutation"]
pub struct MutationGuard {
    _operation_lock: OperationLock,
}

impl MutationGuard {
    pub fn acquire() -> Result<Self, MutationGuardError> {
        #[cfg(test)]
        return Self::acquire_with(|| Ok(false));

        #[cfg(not(test))]
        Self::acquire_with(game_process::is_game_running)
    }

    fn acquire_with(
        is_game_running: impl FnOnce() -> Result<bool, GameProcessError>,
    ) -> Result<Self, MutationGuardError> {
        let operation_lock =
            OperationLock::try_acquire()?.ok_or(MutationGuardError::OperationInProgress)?;

        if is_game_running()? {
            return Err(MutationGuardError::GameRunning);
        }

        Ok(Self {
            _operation_lock: operation_lock,
        })
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn acquires_when_no_other_operation_or_game_is_running() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();

        let guard = MutationGuard::acquire_with(|| Ok(false)).unwrap();

        drop(guard);
    }

    #[test]
    fn reports_a_running_game_as_a_distinct_block_reason() {
        let _test = MUTATION_GUARD_TEST.lock().unwrap();

        let result = MutationGuard::acquire_with(|| Ok(true));

        assert!(matches!(result, Err(MutationGuardError::GameRunning)));
    }
}
