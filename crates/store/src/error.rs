use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Decode(#[from] tlmbench_decode::Error),
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: std::io::Error },
    #[error("{}: the cache entry is damaged (delete it to rebuild): {why}", path.display())]
    Damaged { path: PathBuf, why: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// For `map_err`: an I/O error at `path`.
    pub(crate) fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
        move |source| Error::Io { path: path.to_owned(), source }
    }
}
