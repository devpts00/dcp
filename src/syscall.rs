use std::io::Write;
use std::os::fd::AsRawFd;
use faststr::FastStr;
use nix::ioctl_write_ptr;
use tracing::{info, instrument};
use crate::error::DcpError;
use crate::io::{check_size_or_errno, open_file, Mode};
use crate::util::show_progress;

ioctl_write_ptr!(set_flags, b'f', 2, i32);

#[instrument(level = "debug")]
pub fn syscall_copy(src: FastStr, dst: FastStr, direct: bool, chunk_size: u32) -> Result<u64, DcpError> {

    let src_meta = std::fs::metadata(src.as_str())?;
    let file_size = src_meta.len() as libc::size_t;
    info!("file_size: {}", file_size);

    let flags = if direct { libc::O_DIRECT } else { 0 };
    let read_file = open_file(&src, Mode::Read, flags, libc::POSIX_FADV_SEQUENTIAL)?;
    let mut write_file = open_file(&dst, Mode::Write, flags, libc::POSIX_FADV_SEQUENTIAL)?;

    unsafe {
        const FS_NOCOW_FL: i32 = 0x00800000;
        let mut flags: libc::c_int = 0;
        libc::ioctl(write_file.as_raw_fd(), libc::FS_IOC_GETFLAGS, &mut flags);
        flags |= FS_NOCOW_FL;
        libc::ioctl(write_file.as_raw_fd(), libc::FS_IOC_SETFLAGS, &mut flags);
    };

    let copy_size = 16 * 1024 * 1024;

    let null = std::ptr::null_mut();
    let mut write_size = 0;
    let mut progress = 0;
    while write_size < file_size {
        let res = unsafe {
            libc::copy_file_range(read_file.as_raw_fd(), null, write_file.as_raw_fd(), null, copy_size, 0)
        };
        let size = check_size_or_errno(res)?;
        write_size = write_size + size;
        show_progress(&mut progress, (100 * write_size / file_size) as u64);
    }

    info!("written: {}", write_size);

    write_file.flush()?;

    Ok(write_size as u64)
}