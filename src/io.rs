use std::fs::File;
use std::os::fd::AsRawFd;
use std::ptr::write_bytes;
use faststr::FastStr;
use io_uring::IoUring;
use memmap2::{Advice, MmapMut};
use std::os::unix::prelude::OpenOptionsExt;
use libc::posix_fadvise64;
use tracing::{debug, instrument};
use crate::error::DcpError;



pub struct Buffer {
    mmap: MmapMut,
    ptr: *mut u8,
    size: usize,
}

// pub struct Buffers {
//     bufs: Vec<*mut u8>,
//     mmap: MmapMut,
//     size: usize,
// }

impl Buffer {
    pub fn new(size: u32, align: u32) -> Result<Self, std::io::Error> {
        debug!("alloc, size: {}, align: {}", size, align);
        let size = size as usize;
        let align = align as usize;
        let total: usize = size + align;
        let mut mmap = MmapMut::map_anon(total)?;
        //mmap.advise(Advice::WillNeed)?;
        //mmap.lock()?;
        let ptr_base = mmap.as_mut_ptr();
        unsafe { write_bytes(ptr_base, 0, total); }
        let ptr = ptr_base.map_addr(|p| (p + align - 1) & !(align - 1));
        debug!("ptr, total: {}, base: {:?}, aligned: {:?}", total, ptr_base, ptr);
        Ok(Buffer { mmap, ptr, size })
    }
    pub fn as_iovec(&self) -> libc::iovec {
        let iov_base = self.ptr as *mut libc::c_void;
        let iov_len = self.size as libc::size_t;
        libc::iovec { iov_base, iov_len }
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }
    pub fn as_ptr_mut(&self) -> *mut u8 {
        self.ptr
    }
}

#[inline]
pub fn check_size_or_error(res: i32) -> Result<u32, std::io::Error> {
    if res < 0 {
        Err(std::io::Error::from_raw_os_error(-res))
    } else {
        Ok(res as u32)
    }
}

pub fn check_size_or_errno(res: isize) -> Result<usize, std::io::Error> {
    if res < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(res as usize)
    }
}

#[inline]
pub fn submit(ring: &mut IoUring, sqe: io_uring::squeue::Entry) -> Result<(), DcpError> {
    unsafe { ring.submission().push(&sqe)? };
    ring.submit()?;
    Ok(())
}

#[inline]
pub fn poll(ring: &mut IoUring) -> io_uring::cqueue::Entry {
    loop {
        if let Some(cqe) = ring.completion().next() {
            return cqe;
        }
    }
}


#[derive(Debug)]
pub enum Mode {
    Read, Write
}

#[instrument(level="debug")]
pub fn open_file(path: &FastStr, mode: Mode, flags: libc::c_int, advise: libc::c_int) -> Result<File, std::io::Error> {
    let mut options = File::options();
    match mode {
        Mode::Read => {
            options.read(true);
        }
        Mode::Write => {
            options.write(true).create(true).truncate(true);
        }
    }
    if flags != 0 {
        options.custom_flags(flags);
    }
    let file = options.open(path.as_str())?;
    if advise != 0 {
        unsafe {
            posix_fadvise64(file.as_raw_fd(), 0, 0, advise);
        }
    }
    Ok(file)
}
