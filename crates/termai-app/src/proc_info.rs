//! Query a process's current working directory by PID.
//!
//! Shells don't reliably emit OSC 7 (macOS's default zsh doesn't), so we can't
//! depend on the terminal parser knowing the cwd. Instead we ask the OS for the
//! shell child's cwd directly — it changes as the user `cd`s, so it's accurate
//! at the prompt. macOS uses `proc_pidinfo(PROC_PIDVNODEPATHINFO)` from
//! libSystem (no extra crate, no bindgen). Other platforms return `None` for
//! now (callers fall back to OSC 7 / the app's cwd).

use std::path::PathBuf;

#[cfg(target_os = "macos")]
pub fn pid_cwd(pid: u32) -> Option<PathBuf> {
    use std::os::raw::{c_char, c_int, c_void};

    // PROC_PIDVNODEPATHINFO returns a `proc_vnodepathinfo` (2352 bytes): two
    // `vnode_info_path`s (cdir, rdir), each = 152-byte vnode_info + 1024-byte
    // path. We only read the first path (the current directory).
    const PROC_PIDVNODEPATHINFO: c_int = 9;
    const MAXPATHLEN: usize = 1024;

    #[repr(C)]
    struct VnodeInfoPath {
        vip_vi: [u8; 152],
        vip_path: [c_char; MAXPATHLEN],
    }
    #[repr(C)]
    struct ProcVnodePathInfo {
        pvi_cdir: VnodeInfoPath,
        pvi_rdir: VnodeInfoPath,
    }

    unsafe extern "C" {
        fn proc_pidinfo(
            pid: c_int,
            flavor: c_int,
            arg: u64,
            buffer: *mut c_void,
            buffersize: c_int,
        ) -> c_int;
    }

    unsafe {
        let mut info: ProcVnodePathInfo = std::mem::zeroed();
        let size = std::mem::size_of::<ProcVnodePathInfo>() as c_int;
        let n = proc_pidinfo(
            pid as c_int,
            PROC_PIDVNODEPATHINFO,
            0,
            &mut info as *mut _ as *mut c_void,
            size,
        );
        // Success returns the number of bytes written (== size).
        if n < size {
            return None;
        }
        let cstr = std::ffi::CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr());
        let s = cstr.to_str().ok()?;
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn pid_cwd(_pid: u32) -> Option<PathBuf> {
    None
}
