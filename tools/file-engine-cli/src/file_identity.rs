use std::path::Path;

/// Returns the operating-system-backed identity for a regular file when the
/// platform exposes one. Callers must treat `None` as "identity unavailable",
/// not as a synthetic success value.
#[cfg(windows)]
pub fn file_id_for_path(path: &Path) -> Option<String> {
    use std::fs::File;
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let file = File::open(path).ok()?;
    let mut info = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, info.as_mut_ptr()) };
    if ok == 0 {
        return None;
    }
    let info = unsafe { info.assume_init() };
    let file_index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    Some(format!(
        "win:v{:08x}:i{file_index:016x}",
        info.dwVolumeSerialNumber
    ))
}

#[cfg(unix)]
pub fn file_id_for_path(path: &Path) -> Option<String> {
    use std::fs;
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(path).ok()?;
    Some(format!(
        "unix:d{:016x}:i{:016x}",
        metadata.dev(),
        metadata.ino()
    ))
}

#[cfg(not(any(unix, windows)))]
pub fn file_id_for_path(_path: &Path) -> Option<String> {
    None
}
