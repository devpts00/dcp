mod args;
mod common;
mod error;
mod iouring;
mod stream;
mod syscall;

use crate::args::Cmd;
use crate::error::DcpError;
use crate::iouring::io_uring_copy;
use crate::stream::stream_copy;
use crate::syscall::syscall_copy;
use crate::common::{init_tracing, log};
use clap::Parser;
use std::time::Instant;
use tracing::{debug, info};

fn run(cmd: Cmd) -> Result<u64, DcpError> {
    match cmd {
        Cmd::IoUring { direct, poll_ms, buffer_size, buffer_count, files } => {
            io_uring_copy(files.src, files.dst, direct, poll_ms, buffer_size.as_u64() as u32, buffer_count)
        }
        Cmd::Stream { buffer_size, direct, files } => {
            stream_copy(files.src, files.dst,direct, buffer_size.as_u64() as u32)
        }
        Cmd::Syscall { files, direct, chunk_size } => {
            syscall_copy(files.src, files.dst, direct, chunk_size.as_u64() as u32)
        }
    }
}

fn main() {
    let _guard = init_tracing();
    let cmd = args::Cmd::parse();
    debug!("cmd: {:?}", cmd);
    let start = Instant::now();
    log(run(cmd));
    debug!("elapsed: {} seconds", start.elapsed().as_secs_f64());
}
