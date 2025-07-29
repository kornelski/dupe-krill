use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LinkType {
    Hardlink,
    Reflink,
}

/// Create a reflink (copy-on-write link) between two files
/// Falls back to hardlinking if reflinking is not supported
pub fn reflink_or_hardlink(src: &Path, dst: &Path) -> io::Result<LinkType> {
    // Try reflink first
    match reflink(src, dst) {
        Ok(()) => Ok(LinkType::Reflink),
        Err(_) => {
            // Fall back to hardlink
            fs::hard_link(src, dst)?;
            Ok(LinkType::Hardlink)
        }
    }
}

/// Create a reflink (copy-on-write link) between two files
pub fn reflink(src: &Path, dst: &Path) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        reflink_linux(src, dst)
    }
    #[cfg(target_os = "macos")]
    {
        reflink_macos(src, dst)
    }
    #[cfg(target_os = "windows")]
    {
        reflink_windows(src, dst)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Reflinks are not supported on this platform",
        ))
    }
}

#[cfg(target_os = "linux")]
fn reflink_linux(src: &Path, dst: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let src_c = CString::new(src.as_os_str().as_bytes())?;
    let dst_c = CString::new(dst.as_os_str().as_bytes())?;

    unsafe {
        // First try ioctl FICLONE (Btrfs, XFS)
        let src_fd = libc::open(src_c.as_ptr(), libc::O_RDONLY);
        if src_fd == -1 {
            return Err(io::Error::last_os_error());
        }

        let dst_fd = libc::open(dst_c.as_ptr(), libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL, 0o644);
        if dst_fd == -1 {
            libc::close(src_fd);
            return Err(io::Error::last_os_error());
        }

        // FICLONE ioctl constant - this creates a reflink
        const FICLONE: libc::c_ulong = 0x40049409;
        let result = libc::ioctl(dst_fd, FICLONE, src_fd);
        
        libc::close(src_fd);
        libc::close(dst_fd);

        if result == 0 {
            Ok(())
        } else {
            // Clean up the created file on failure
            let _ = libc::unlink(dst_c.as_ptr());
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(target_os = "macos")]
fn reflink_macos(src: &Path, dst: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let src_c = CString::new(src.as_os_str().as_bytes())?;
    let dst_c = CString::new(dst.as_os_str().as_bytes())?;

    unsafe {
        // Use clonefile() on macOS
        extern "C" {
            fn clonefile(src: *const libc::c_char, dst: *const libc::c_char, flags: u32) -> libc::c_int;
        }

        let result = clonefile(src_c.as_ptr(), dst_c.as_ptr(), 0);
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(target_os = "windows")]
fn reflink_windows(src: &Path, dst: &Path) -> io::Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    // Convert paths to wide strings for Windows API
    let src_wide: Vec<u16> = src.as_os_str().encode_wide().chain(Some(0)).collect();
    let dst_wide: Vec<u16> = dst.as_os_str().encode_wide().chain(Some(0)).collect();

    unsafe {
        // Windows doesn't have a direct equivalent to FICLONE, but it's possible to
        // use the CopyFile API with COPY_FILE_COPY_SYMLINK | COPY_FILE_CLONE_FORCE.
        // This requires Windows 10 version 1903 or later with a ReFS filesystem
        
        extern "system" {
            fn CopyFileExW(
                lpExistingFileName: *const u16,
                lpNewFileName: *const u16,
                lpProgressRoutine: *const u8,
                lpData: *const u8,
                pbCancel: *const i32,
                dwCopyFlags: u32,
            ) -> i32;
        }

        // COPY_FILE_CLONE_FORCE = 0x00800000 - Force a clone (reflink)
        const COPY_FILE_CLONE_FORCE: u32 = 0x00800000;
        
        let result = CopyFileExW(
            src_wide.as_ptr(),
            dst_wide.as_ptr(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            COPY_FILE_CLONE_FORCE,
        );

        if result != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
