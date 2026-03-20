use std::alloc::Layout;
use std::ops::{Index, IndexMut};
use crate::error::DcpError;
use io_buffer::Buffer;
use io_uring::IoUring;
use nix::errno::Errno;
use tracing::debug;
use crate::util::allocate;

pub struct IoVec(Buffer);

impl IoVec {
    pub fn new(size: u32, align: u32) -> Result<Self, Errno> {
        debug!("alloc, size: {}, align: {}", size, align);
        let buf = Buffer::aligned_by(size as i32, align as u32)?;
        Ok(IoVec(buf))
    }
    pub fn as_iovec(&mut self) -> libc::iovec {
        let iov_base = self.0.get_raw_mut() as *mut libc::c_void;
        let iov_len = self.0.len() as libc::size_t;
        libc::iovec { iov_base, iov_len }
    }
    pub fn as_raw_mut(&mut self) -> *mut u8 {
        self.0.get_raw_mut()
    }
    pub fn len(&self) -> u32 {
        self.0.len() as u32
    }
}

pub struct Buffers {
    bufs: Vec<*mut u8>,
    layout: Layout
}

impl Buffers {
    pub fn new(size: u32, align: u32, count: u16) -> Result<Self, DcpError> {
        debug!("alloc, size: {}, align: {}, count: {}", size, align, count);
        let layout = Layout::from_size_align(size as usize, align as usize)?;
        let mut bufs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let ptr = allocate(layout)?;
            bufs.push(ptr);
        }
        Ok(Buffers { bufs, layout })
    }
    pub fn register(&mut self, ring: &mut IoUring) -> std::io::Result<()> {
        let io_vecs: Vec<libc::iovec> = self.bufs.iter_mut().map(|ptr| {
            let iov_base = *ptr as *mut libc::c_void;
            let iov_len = self.layout.size() as libc::size_t;
            libc::iovec { iov_base, iov_len }
        }).collect();
        unsafe {
            ring.submitter().register_buffers(io_vecs.as_slice())
        }
    }
}

impl Index<u16> for Buffers {
    type Output = *mut u8;
    fn index(&self, index: u16) -> &Self::Output {
        self.bufs.index(index as usize)
    }
}

impl IndexMut<u16> for Buffers {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        self.bufs.index_mut(index as usize)
    }
}

impl Drop for Buffers {
    fn drop(&mut self) {
        for buf in &self.bufs {
            unsafe {
                std::alloc::dealloc(*buf, self.layout)
            }
        }
    }
}

#[inline]
pub fn check(res: i32) -> Result<i32, std::io::Error> {
    if res < 0 {
        Err(std::io::Error::from_raw_os_error(-res))
    } else {
        Ok(res)
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
