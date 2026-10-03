use std::fmt;

#[derive(Debug)]
pub enum Error {
    InvalidQuery(String),
    TimedOut,
    Backend(anyhow::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery(message) => f.write_str(message),
            Self::TimedOut => f.write_str("the query ran past its time limit"),
            Self::Backend(error) => write!(f, "{error:#}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend(error) => error.source(),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(error: anyhow::Error) -> Self {
        Self::Backend(error)
    }
}
