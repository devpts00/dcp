use std::alloc::{handle_alloc_error, Layout, LayoutErr};
use std::error::Error;
use std::os::fd::RawFd;
use io_uring::types::Fd;
use libc::aligned_alloc;
use tracing::error;
use tracing::level_filters::LevelFilter;
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
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
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
pub fn log<E: Error>(result: Result<(), E>) {
    if let Err(err) = result {
        error!("error: {}", err);
    }
}

#[inline]
pub fn allocate(layout: Layout) -> Result<*mut u8, std::io::Error> {
    let ptr = unsafe {
        std::alloc::alloc(layout)
    };
    if ptr.is_null() {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(ptr)
    }
}
