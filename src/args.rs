use bytesize::ByteSize;
use clap::{Args, Parser};
use faststr::FastStr;


#[derive(Args, Debug)]
pub struct Files {
    #[arg(long)]
    pub src: FastStr,
    #[arg(long)]
    pub dst: FastStr,
}

#[derive(Parser, Debug)]
#[command(version, about)]
pub enum Cmd {
    IoUring {
        #[command(flatten)]
        files: Files,
        #[arg(long)]
        direct: bool,
        #[arg(long)]
        poll_ms: Option<u32>,
        #[arg(long)]
        buffer_size: ByteSize,
        #[arg(long)]
        buffer_count: u8,
    },
    Stream {
        #[command(flatten)]
        files: Files,
        #[arg(long)]
        direct: bool,
        #[arg(long)]
        buffer_size: ByteSize,
    },
    Syscall {
        #[command(flatten)]
        files: Files,
        #[arg(long)]
        direct: bool,
        #[arg(long)]
        chunk_size: ByteSize,
    }
}
