//! Closed main binding. No path, File, Connection or raw HANDLE public getter.
//! Unix: named live-file witness only, not fstat of SQLite's own descriptor.
//! Windows: actual live SQLite handle must agree with the named live-file anchor.
//! Both require controlled stable deployment; active replace/restore is excluded.
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
};
#[derive(PartialEq, Eq)]
enum Identity {
    #[cfg(unix)]
    Unix { device: u64, inode: u64 },
    #[cfg(windows)]
    Windows { volume: u64, id: [u8; 16] },
}
pub(crate) struct MainSource {
    path: Arc<PathBuf>,
    identity: Identity,
    _anchor: Arc<File>,
}
impl Clone for MainSource {
    fn clone(&self) -> Self {
        Self {
            path: Arc::clone(&self.path),
            identity: self.copy_identity(),
            _anchor: Arc::clone(&self._anchor),
        }
    }
}
impl PartialEq for MainSource {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.identity == other.identity
    }
}
impl Eq for MainSource {}
impl MainSource {
    fn copy_identity(&self) -> Identity {
        match &self.identity {
            #[cfg(unix)]
            Identity::Unix { device, inode } => Identity::Unix {
                device: *device,
                inode: *inode,
            },
            #[cfg(windows)]
            Identity::Windows { volume, id } => Identity::Windows {
                volume: *volume,
                id: *id,
            },
        }
    }
    // Named-file rechecks do not claim evidence from an SQLite native handle.
    pub(super) fn capture(name: Option<&str>) -> Option<Self> {
        let name = name.filter(|s| !s.is_empty())?;
        if name == ":memory:" || name.starts_with("file:") {
            return None;
        }
        let path = std::fs::canonicalize(Path::new(name)).ok()?;
        let file = File::open(&path).ok()?;
        let meta = file.metadata().ok()?;
        if !meta.is_file() {
            return None;
        }
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            Identity::Unix {
                device: meta.dev(),
                inode: meta.ino(),
            }
        };
        #[cfg(windows)]
        let identity = {
            use std::os::windows::io::AsRawHandle;
            windows_identity(file.as_raw_handle())?
        };
        #[cfg(any(unix, windows))]
        {
            Some(Self {
                path: Arc::new(path),
                identity,
                _anchor: Arc::new(file),
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (path, file);
            None
        }
    }
    // Called only synchronously by the sole worker while Connection/Tx is live.
    pub(super) fn capture_native(conn: &crate::rusqlite::Connection) -> Option<Self> {
        let named = Self::capture(conn.path())?;
        #[cfg(windows)]
        {
            use crate::rusqlite::ffi;
            use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
            let mut handle: HANDLE = std::ptr::null_mut();
            // SAFETY: worker owns this live Connection, no concurrent close or
            // SQLite calls. Documented file-control writes one borrowed HANDLE
            // into initialized local storage; no VFS-layout cast or handle close.
            let result = unsafe {
                ffi::sqlite3_file_control(
                    conn.handle(),
                    c"main".as_ptr(),
                    ffi::SQLITE_FCNTL_WIN32_GET_HANDLE,
                    (&mut handle as *mut HANDLE).cast(),
                )
            };
            if result != ffi::SQLITE_OK || handle.is_null() || handle == INVALID_HANDLE_VALUE {
                return None;
            }
            if windows_identity(handle)? != named.identity {
                return None;
            }
        }
        Some(named)
    }
    pub(super) fn canonical_path(&self) -> PathBuf {
        self.path.as_ref().clone()
    }
    pub(super) fn still_same(&self) -> bool {
        Self::capture(self.path.to_str()).is_some_and(|s| s == *self)
    }
}
#[cfg(windows)]
fn windows_identity(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<Identity> {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        FileIdInfo, GetFileInformationByHandleEx, FILE_ID_INFO,
    };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return None;
    }
    let mut info = FILE_ID_INFO::default();
    // SAFETY: caller lends a currently live File/worker SQLite HANDLE for this
    // synchronous call. Exact official struct/size, initialized writable output.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if ok == 0 {
        let _redacted = std::io::Error::last_os_error();
        return None;
    }
    Some(Identity::Windows {
        volume: info.VolumeSerialNumber,
        id: info.FileId.Identifier,
    })
}
#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn full_identity_compares_high_half() {
        let low = super::Identity::Windows {
            volume: 1,
            id: [0; 16],
        };
        let mut high = [0; 16];
        high[15] = 1;
        assert!(
            low != super::Identity::Windows {
                volume: 1,
                id: high
            }
        );
    }
}
