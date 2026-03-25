use crate::error::DcpError;
use faststr::FastStr;
use num_integer::lcm;
use std::alloc::{dealloc, Layout};
use std::error::Error;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::prelude::OpenOptionsExt;
use std::os::linux::fs::MetadataExt;
use libc::posix_fadvise64;
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, instrument};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer()
            .pretty()
            .with_file(false)
            .with_line_number(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
            .with_filter(
                EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .from_env()
                    .unwrap()
            )
        )
        .init();
}

#[inline]
pub fn check_size_or_error(res: i32) -> Result<u32, std::io::Error> {
    if res < 0 {
        Err(std::io::Error::from_raw_os_error(-res))
    } else {
        Ok(res as u32)
    }
}

#[inline]
pub fn check_size_or_errno(res: isize) -> Result<usize, std::io::Error> {
    if res < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(res as usize)
    }
}

#[inline]
pub fn log<T, E: Error>(result: Result<T, E>) {
    if let Err(err) = result {
        error!("error: {}", err);
    }
}

#[inline]
pub fn show_progress(prev: &mut u64, curr: u64) {
    if curr > *prev {
        *prev = curr;
        debug!("progress: {}%", curr);
    }
}

#[inline]
pub unsafe fn allocate(layout: Layout) -> Result<*mut u8, std::io::Error> {
    let ptr = std::alloc::alloc(layout);
    if ptr.is_null() {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(ptr)
    }
}

#[inline]
pub unsafe fn deallocate(ptr: *mut u8, layout: Layout) {
    dealloc(ptr, layout);
}

#[instrument(level="debug")]
pub fn calc_sizes(src: &FastStr, dst: &FastStr) -> Result<(u64, u32), DcpError> {
    let src_meta = std::fs::metadata(src.as_str())?;
    let src_block_size = src_meta.st_blksize() as u32;
    let src_file_size = src_meta.len();
    debug!("src, block: {}, length: {}", src_block_size, src_file_size);

    let dst_meta = std::fs::metadata(dst.as_str())?;
    let dst_block_size = dst_meta.st_blksize() as u32;
    let dst_file_size = dst_meta.len();
    debug!("dst, block: {}, length: {}", dst_block_size, dst_file_size);

    let block_size = lcm(src_block_size, dst_block_size);
    debug!("buf, block: {}", block_size);
    Ok((src_file_size, block_size))
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
