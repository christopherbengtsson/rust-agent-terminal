//! Bounded user-facing cleanup records and state-filesystem capacity sampling.
use serde::{Deserialize, Serialize};
use std::{ffi::CString, io, os::unix::ffi::OsStrExt, path::Path};
pub const LOW_SPACE: u64 = 1024 * 1024 * 1024;
pub const LIST_SIZE: usize = 8;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Capacity {
    pub available: Option<u64>,
    pub error: Option<String>,
}
impl Capacity {
    pub fn low(&self) -> bool {
        self.available.is_some_and(|bytes| bytes < LOW_SPACE)
    }
    pub fn sample(directory: &Path) -> Self {
        let result = (|| {
            #[cfg(debug_assertions)]
            if let Ok(value) = std::fs::read_to_string(directory.join("proof-storage-available")) {
                return value.trim().parse::<u64>().map_err(io::Error::other);
            }
            let path = CString::new(directory.as_os_str().as_bytes()).map_err(io::Error::other)?;
            let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
            // SAFETY: path is NUL terminated; stat points to valid writable storage.
            if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: a successful statvfs initialized the output.
            let stat = unsafe { stat.assume_init() };
            Ok((stat.f_bavail as u128 * stat.f_frsize as u128).min(u64::MAX as u128) as u64)
        })();
        match result {
            Ok(bytes) => Self {
                available: Some(bytes),
                error: None,
            },
            Err(error) => Self {
                available: None,
                error: Some(error.to_string()),
            },
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub identity: String,
    pub checkout: String,
    pub created: Option<u64>,
    pub bytes: Option<u64>,
    pub eligible: bool,
    pub status: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Page {
    pub index: usize,
    pub next: bool,
    pub entries: Vec<Entry>,
}
