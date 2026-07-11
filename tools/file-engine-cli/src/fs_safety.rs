use std::fs::{FileType, Metadata};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

/// Returns true for filesystem entries the engine must not traverse or expose as normal files.
/// On Windows, junctions and other reparse points are not always reported as simple symlinks, so
/// we also check the raw reparse attribute before recursing into a directory.
pub fn is_link_or_reparse_point(metadata: &Metadata, file_type: FileType) -> bool {
    file_type.is_symlink() || is_windows_reparse_point(metadata)
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &Metadata) -> bool {
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &Metadata) -> bool {
    false
}
