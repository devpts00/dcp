use crate::error::DcpError;
use crate::common::{allocate, calc_sizes, deallocate, open_file, show_progress, Mode};
use faststr::FastStr;
use std::alloc::Layout;
use std::io::{Read, Write};
use std::slice::from_raw_parts_mut;
use thousands::Separable;
use tracing::{debug, instrument, trace};

#[instrument(level="debug")]
pub fn stream_copy(src: FastStr, dst: FastStr, direct: bool, buffer_size: u32) -> Result<u64, DcpError> {

    let flags = if direct { libc::O_DIRECT } else { 0 };
    let mut read_file = open_file(&src, Mode::Read, flags, libc::POSIX_FADV_SEQUENTIAL)?;
    let mut write_file = open_file(&dst, Mode::Write, flags, libc::POSIX_FADV_SEQUENTIAL)?;

    let (file_size, block_size) = calc_sizes(&src, &dst)?;
    let layout = Layout::from_size_align(buffer_size as usize, block_size as usize)?;
    let ptr = unsafe { allocate(layout) }?;
    let mut buf = unsafe { from_raw_parts_mut(ptr, buffer_size as usize) };

    let mut progress = 0;

    let mut write_size = 0;
    loop {
        let n = read_file.read(&mut buf)?;
        trace!("read {}", n.separate_with_commas());
        if n == 0 {
            break;
        }
        write_file.write(&buf[..n])?;
        write_size += n as u64;
        show_progress(&mut progress, 100 * write_size / file_size);
    }
    write_file.flush()?;
    debug!("size: {}, copied: {}", file_size, write_size);

    unsafe { deallocate(ptr, layout); }

    Ok(write_size)
}