use std::alloc::Layout;
use crate::error::DcpError;
use crate::common::{allocate, calc_sizes, check_size_or_error, deallocate, show_progress};
use faststr::FastStr;
use io_uring::types::{Fd, Fixed};
use io_uring::{opcode, types, IoUring};
use std::collections::VecDeque;
use std::os::fd::RawFd;
use io_uring::squeue::Flags;
use thousands::Separable;
use tracing::{debug, instrument, trace};

pub struct Buffer {
    ptr: *mut u8,
    layout: Layout,
}

impl Buffer {
    pub fn new(size: usize, align: usize) -> Result<Buffer, DcpError> {
        debug!("alloc, size: {}, align: {}", size, align);
        let layout = Layout::from_size_align(size, align)?;
        let ptr = unsafe { allocate(layout) }?;
        Ok(Buffer { ptr, layout })
    }
    pub fn as_iovec(&self) -> libc::iovec {
        let iov_base = self.ptr as *mut libc::c_void;
        let iov_len = self.layout.size() as libc::size_t;
        libc::iovec { iov_base, iov_len }
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }
    pub fn as_ptr_mut(&self) -> *mut u8 {
        self.ptr
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe { deallocate(self.ptr, self.layout); }
    }
}

#[inline]
fn submit(ring: &mut IoUring, sqe: io_uring::squeue::Entry) -> Result<(), DcpError> {
    unsafe { ring.submission().push(&sqe)? };
    ring.submit()?;
    Ok(())
}

#[inline]
fn poll(ring: &mut IoUring) -> io_uring::cqueue::Entry {
    loop {
        if let Some(cqe) = ring.completion().next() {
            return cqe;
        }
    }
}

#[instrument(level="debug", skip(ring))]
fn open(ring: &mut IoUring, path: &FastStr, flags: libc::c_int) -> Result<types::Fd, DcpError> {
    let cwd = types::Fd(libc::AT_FDCWD);
    let path = std::ffi::CString::new(path.as_str())?;
    let sqe = opcode::OpenAt::new(cwd, path.as_ptr())
        .flags(flags)
        .build();
    submit(ring, sqe)?;
    let cqe = poll(ring);
    let res = check_size_or_error(cqe.result())?;
    Ok(Fd(res as RawFd))
}

#[instrument(level="debug", skip(ring))]
fn close(ring: &mut IoUring, fd: Fd) -> Result<(), DcpError> {
    let sqe = opcode::Close::new(fd).build();
    submit(ring, sqe)?;
    let cqe = poll(ring);
    check_size_or_error(cqe.result())?;
    Ok(())
}

#[instrument(level="debug", skip(ring))]
fn fadvise(ring: &mut IoUring, ffd: Fixed, len: u32, flags: i32) -> Result<(), DcpError> {
    let sqe = opcode::Fadvise::new(ffd, len as libc::off_t, flags).build();
    submit(ring, sqe)?;
    let cqe = poll(ring);
    check_size_or_error(cqe.result())?;
    Ok(())
}

#[instrument(level="debug", skip(ring))]
fn fsync(ring: &mut IoUring, ffd: Fixed) -> Result<(), DcpError> {
    let sqe = opcode::Fsync::new(ffd).build();
    submit(ring, sqe)?;
    let cqe = poll(ring);
    check_size_or_error(cqe.result())?;
    Ok(())
}

#[instrument(level="debug", skip(ring))]
fn ftruncate(ring: &mut IoUring, ffd: Fixed, size: u64) -> Result<(), DcpError> {
    let sqe = opcode::Ftruncate::new(ffd, size).build();
    submit(ring, sqe)?;
    let cqe = poll(ring);
    check_size_or_error(cqe.result())?;
    Ok(())
}

#[instrument(level="debug")]
fn create_io_uring(capacity: u8, poll: Option<u32>) -> Result<IoUring, DcpError> {
    let mut builder = IoUring::builder();
    if let Some(poll) = poll {
        builder.setup_sqpoll(poll);
    }
    let ring = builder.build(capacity as u32)?;
    Ok(ring)
}

#[instrument(level="debug", skip(ring))]
fn register_file(ring: &mut IoUring, fd: Fd) -> Result<Fixed, DcpError> {
    ring.submitter().register_files(&[fd.0])?;
    Ok(Fixed(0))
}

#[instrument(level="debug")]
fn create_buffers(size: u32, align: u32, count: u8) -> Result<Vec<Buffer>, DcpError> {
    let mut buffers = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let buf = Buffer::new(size as usize, align as usize)?;
        buffers.push(buf);
    }
    Ok(buffers)
}

#[instrument(level="debug", skip(ring, buffers))]
fn register_buffers(ring: &mut IoUring, buffers: &Vec<Buffer>, fill: bool) -> Result<VecDeque<usize>, DcpError> {
    let iovecs: Vec<libc::iovec> = buffers.iter().map(|buf| buf.as_iovec()).collect();
    unsafe {
        ring.submitter().register_buffers(&iovecs)?;
    }
    let indices = if fill {
        (0..buffers.len()).collect()
    } else {
        VecDeque::with_capacity(buffers.len())
    };
    Ok(indices)
}

#[instrument(level="debug", skip(ring))]
fn unregister_buffers(ring: &mut IoUring) -> Result<(), DcpError> {
    ring.submitter().unregister_buffers()?;
    Ok(())
}

#[instrument(level="debug", skip(ring))]
fn unregister_files(ring: &mut IoUring) -> Result<(), DcpError> {
    ring.submitter().unregister_files()?;
    Ok(())
}

#[instrument(level="debug")]
pub fn io_uring_copy(src: FastStr, dst: FastStr, direct: bool, poll_ms: Option<u32>, buffer_size: u32, buffer_count: u8) -> Result<u64, DcpError> {

    let mut read_ring = create_io_uring(buffer_count, poll_ms)?;
    let mut write_ring = create_io_uring(buffer_count, poll_ms)?;

    let flag = if direct { libc::O_DIRECT } else { 0 };
    let read_fd = open(&mut read_ring, &src, libc::O_RDONLY | flag)?;
    let write_fd = open(&mut write_ring, &dst, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | flag)?;

    let (file_size, block_size) = calc_sizes(&src, &dst)?;

    let read_ffd = register_file(&mut read_ring, read_fd)?;
    let write_ffd = register_file(&mut write_ring, write_fd)?;

    fadvise(&mut read_ring, read_ffd, 0, libc::POSIX_FADV_SEQUENTIAL)?;
    fadvise(&mut write_ring, write_ffd, 0, libc::POSIX_FADV_SEQUENTIAL)?;

    let buffers = create_buffers(buffer_size, block_size, buffer_count)?;
    let mut read_indices = register_buffers(&mut read_ring, &buffers, true)?;
    let mut write_indices = register_buffers(&mut write_ring, &buffers, false)?;

    let mut read_offset: u64 = 0;
    let mut write_offset: u64 = 0;
    let mut read_size: u64 = 0;
    let mut write_size: u64 = 0;

    let mut progress = 0;

    while read_size < file_size || write_size < file_size {

        // 1. Submit read if reading is not in progress and vacant buffers are available
        if read_offset < file_size {
            if let Some(idx) = read_indices.pop_front() {
                trace!("submit: read, index: {}, offset: {}", idx, read_offset.separate_with_commas());
                let sqe = opcode::ReadFixed::new(read_ffd, buffers[idx].as_ptr_mut(), buffer_size, idx as u16)
                    .offset(read_offset)
                    .build()
                    .flags(Flags::IO_LINK)
                    .user_data(idx as u64);
                unsafe { read_ring.submission().push(&sqe)? };
                read_ring.submit()?;
                read_offset += buffer_size as u64;
            }
        }

        // 2. Submit write if filled buffers are available
        if write_offset < file_size {
            if let Some(idx) = write_indices.pop_front() {
                trace!("submit: write, index: {}, offset: {}", idx, write_offset.separate_with_commas());
                let sqe = opcode::WriteFixed::new(write_ffd, buffers[idx].as_ptr(), buffer_size, idx as u16)
                    .offset(write_offset)
                    .build()
                    .flags(Flags::IO_LINK)
                    .user_data(idx as u64);
                unsafe { write_ring.submission().push(&sqe)? };
                write_ring.submit()?;
                write_offset += buffer_size as u64;
            }
        }

        // 3. Collect read completion if any
        while let Some(cqe) = read_ring.completion().next() {
            let size = check_size_or_error(cqe.result())? as u64;
            let idx = cqe.user_data() as usize;
            write_indices.push_back(idx);
            read_size += size;
            trace!("complete: read, index: {}, size: {}, total: {}", idx, size.separate_with_commas(), read_size.separate_with_commas());
        }

        // 4. Collect write completion if any
        while let Some(cqe) = write_ring.completion().next() {
            let size = check_size_or_error(cqe.result())? as u64;
            let idx = cqe.user_data() as usize;
            read_indices.push_back(idx);
            write_size += size;
            trace!("complete: write, index: {}, size: {}, total: {}", idx, size.separate_with_commas(), write_size.separate_with_commas());
            show_progress(&mut progress, 100 * write_size / file_size);
        }
    }

    fsync(&mut write_ring, write_ffd)?;
    ftruncate(&mut write_ring, write_ffd, file_size)?;
    unregister_buffers(&mut write_ring)?;
    unregister_files(&mut write_ring)?;
    close(&mut write_ring, write_fd)?;

    unregister_buffers(&mut read_ring)?;
    unregister_files(&mut read_ring)?;
    close(&mut read_ring, read_fd)?;

    Ok(write_size)
}
