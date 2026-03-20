mod args;
mod util;
mod error;
mod io;

use std::alloc::{alloc, handle_alloc_error, Layout, LayoutError};
use std::collections::VecDeque;
use std::mem::transmute;
use std::os::linux::fs::MetadataExt;
use std::time::Instant;
use bytesize::ByteSize;
use thousands::Separable;
use clap::Parser;
use faststr::FastStr;
use io_uring::{opcode, squeue, types, IoUring};
use io_uring::types::{Fd, Fixed};
use nix::errno::Errno;
use tracing::{debug, info};
use crate::args::Cmd;
use crate::error::DcpError;
use crate::io::{check, poll, submit, Buffers, IoVec};
use crate::util::{init_tracing, log};

fn create_io_vecs(size: u32, align: u32, count: u32) -> Result<Vec<IoVec>, Errno> {
    debug!("create io vecs: {}", count);
    let mut io_vecs = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let io_vec = IoVec::new(size, align)?;
        io_vecs.push(io_vec)
    }
    Ok(io_vecs)
}

fn register_io_vecs(ring: &mut IoUring, io_vecs: &mut Vec<IoVec>) -> std::io::Result<()> {
    let io_vecs: Vec<libc::iovec> = io_vecs.iter_mut()
        .map(|io_vec| io_vec.as_iovec())
        .collect();
    let io_vecs = io_vecs.as_slice();
    unsafe {
        ring.submitter().register_buffers(io_vecs)
    }
}

fn copy(
    ring: &mut IoUring,
    read_ffd: Fixed,
    write_ffd: Fixed,
    file_size: u64,
    block_size: u32,
    buffer_size: u32,
    buffer_count: u32,
) -> Result<u64, DcpError> {

    let write_bit: u64 = 1 << 32;

    // let mut io_vecs = create_io_vecs(buffer_size, block_size, buffer_count)?;
    // register_io_vecs(ring, &mut io_vecs)?;

    let mut buffers = Buffers::new(buffer_size, block_size, buffer_count as u16)?;
    buffers.register(ring)?;

    let mut write_indices = VecDeque::with_capacity(buffer_count as usize);

    let mut read_indices = VecDeque::with_capacity(buffer_count as usize);
    for n in 0..buffer_count as u16 {
        read_indices.push_back(n);
    }

    let mut read_offset: u64 = 0;
    let mut write_offset: u64 = 0;
    let mut read_size: u64 = 0;
    let mut write_size: u64 = 0;

    let mut progress = 0;

    while read_size < file_size || write_size < file_size {

        // 1. Submit read if reading is not in progress and vacant buffers are available
        if read_offset < file_size {
            if let Some(idx) = read_indices.pop_front() {
                debug!("submit: read, index: {}, offset: {}", idx, read_offset.separate_with_commas());
                let sqe = opcode::ReadFixed::new(read_ffd, buffers[idx], buffer_size, idx)
                    .offset(read_offset)
                    .build()
                    .flags(squeue::Flags::IO_LINK)
                    .user_data(idx as u64);
                submit(ring, sqe)?;
                read_offset += buffer_size as u64;
            }
        }

        // 2. Submit write if filled buffers are available
        if write_offset < file_size {
            if let Some(idx) = write_indices.pop_front() {
                debug!("submit: write, index: {}, offset: {}", idx, write_offset.separate_with_commas());
                let sqe = opcode::WriteFixed::new(write_ffd, buffers[idx], buffer_size, idx)
                    .offset(write_offset)
                    .build()
                    .flags(squeue::Flags::IO_LINK)
                    .user_data(idx as u64 | write_bit);
                submit(ring, sqe)?;
                write_offset += buffer_size as u64;
            }
        }

        // 3. Collect completions if any
        while let Some(cqe) = ring.completion().next() {
            let size = check(cqe.result())? as u64;
            let user_data = cqe.user_data();
            if user_data & write_bit == 0 {
                // read
                let idx = user_data as u16;
                write_indices.push_back(idx);
                read_size += size;
                debug!("complete: read, index: {}, size: {}, total: {}", idx, size.separate_with_commas(), read_size.separate_with_commas());
            } else {
                // write
                let idx = (user_data & !write_bit) as u16;
                read_indices.push_back(idx);
                write_size += size;
                debug!("complete: write, index: {}, size: {}, total: {}", idx, size.separate_with_commas(), write_size.separate_with_commas());

                let p = 100 * write_size / file_size;
                if p > progress {
                    progress = p;
                    info!("progress: {}%", progress);
                }
            }
        }
    }

    Ok(write_size)
}

fn open(ring: &mut IoUring, path: FastStr, flags: libc::c_int) -> Result<types::Fd, DcpError> {
    debug!("open, path: {}, flags: {}", path.as_str(), flags);
    let cwd = types::Fd(libc::AT_FDCWD);
    let path = std::ffi::CString::new(path.as_str())?;
    let sqe = opcode::OpenAt::new(cwd, path.as_ptr())
        .flags(flags)
        .build();
    submit(ring, sqe)?;
    let cqe = poll(ring);
    let res = check(cqe.result())?;
    Ok(Fd(res))
}



fn close(ring: &mut IoUring, fd: Fd) -> Result<(), DcpError> {
    debug!("close, fd: {:?}", fd);
    let sqe = opcode::Close::new(fd).build();
    submit(ring, sqe)?;
    let cqe = poll(ring);
    check(cqe.result())?;
    Ok(())
}

fn run(cmd: Cmd) -> Result<(), DcpError> {

    let block_size = cmd.block_size.as_u64() as u32;
    let buffer_size = cmd.buffer_size.as_u64() as u32;
    let buffer_count = cmd.buffer_count as u32;

    let src_meta = std::fs::metadata(cmd.src.as_str())?;
    let file_size = src_meta.len();
    // let block_size = src_meta.st_blksize() as u32;
    // debug!("file size: {}, block size: {}", file_size, block_size);

    debug!("create ring");
    let mut ring = IoUring::builder()
        .setup_sqpoll(1000)
        .build(buffer_count + 1)?;

    let read_fd = open(&mut ring, cmd.src, libc::O_RDONLY | libc::O_DIRECT)?;
    let write_fd = open(&mut ring, cmd.dst, libc::O_WRONLY | libc::O_DIRECT | libc::O_CREAT)?;

    debug!("register file descriptors");
    ring.submitter().register_files(&[read_fd.0, write_fd.0])?;
    let read_ffd = Fixed(0);
    let write_ffd = Fixed(1);

    copy(&mut ring, read_ffd, write_ffd, file_size, block_size, buffer_size, buffer_count)?;

    debug!("unregister buffers");
    ring.submitter().unregister_buffers()?;

    debug!("unregister files");
    ring.submitter().unregister_files()?;

    close(&mut ring, write_fd)?;
    close(&mut ring, read_fd)?;

    Ok(())
}

fn main() {
    let _guard = init_tracing();
    let cmd = args::Cmd::parse();
    info!("cmd: {:?}", cmd);
    let start = Instant::now();
    log(run(cmd));
    info!("elapsed: {} seconds", start.elapsed().as_secs_f64());
}
