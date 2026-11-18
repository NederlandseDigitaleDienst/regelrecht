//! What can go wrong in a cell.

/// An error of the cell.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The law or the chronicle does not give the cell what it needs: two
    /// articles that ask one datum differently, a chronicle line that is no
    /// chronolexogram.
    #[error("setup: {0}")]
    Setup(String),
    /// A request the cell refuses: an article that establishes no
    /// submission, a field the law does not ask, the moment that counts, a
    /// value of the wrong type, a chronolexogram dated before the last.
    #[error("refused: {0}")]
    Refused(String),
    /// The engine failed.
    #[error("engine: {0}")]
    Engine(#[from] regelrecht_engine::EngineError),
    /// A value cannot be stored, read back or compared
    /// ([`regelrecht_law_model::stored`]): a damaged decimal, a record the
    /// engine resolved or computed that holds the decimal key (a supplied
    /// one is refused before), an integer against a decimal of the same
    /// value.
    #[error("value: {0}")]
    Stored(#[from] regelrecht_law_model::stored::StoredError),
    /// Reading or writing the chronicle failed.
    #[error("chronicle: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn setup(message: impl Into<String>) -> Error {
    Error::Setup(message.into())
}

pub(crate) fn refused(message: impl Into<String>) -> Error {
    Error::Refused(message.into())
}
