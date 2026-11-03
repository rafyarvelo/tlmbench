use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },

    #[error(
        "{}: not a format tlmbench reads (CSV, Parquet, a tlmbench reference file, or fixed-width records with a descriptor)",
        .0.display()
    )]
    UnknownFormat(PathBuf),

    #[error("descriptor: {0}")]
    Descriptor(String),

    #[error("{}: {msg}", path.display())]
    Format { path: PathBuf, msg: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn io_err(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Error {
    let path = path.into();
    move |source| Error::Io { path, source }
}

pub(crate) fn format_err(path: impl Into<PathBuf>, msg: impl Into<String>) -> Error {
    Error::Format { path: path.into(), msg: msg.into() }
}
