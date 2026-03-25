use std::alloc::LayoutError;
use nix::errno::Errno;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DcpError {

    #[error("layout: {0}")]
    Layout(#[from] LayoutError),

    #[error("errno: {0}")]
    Errno(#[from] Errno),
    
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("nul: {0}")]
    Nul(#[from] std::ffi::NulError),

    #[error("push: {0}")]
    Push(#[from] io_uring::squeue::PushError),
}
