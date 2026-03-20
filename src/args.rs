use bytesize::ByteSize;
use clap::Parser;
use faststr::FastStr;

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cmd {

    #[arg(long)]
    pub direct: bool,

    #[arg(long)]
    pub poll: bool,

    #[arg(long)]
    pub block_size: ByteSize,

    #[arg(long)]
    pub buffer_size: ByteSize,

    #[arg(long)]
    pub buffer_count: u16,

    pub src: FastStr,
    pub dst: FastStr,
}
