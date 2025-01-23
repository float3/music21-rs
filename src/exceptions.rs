use std::result::Result;
use std::{
    error::Error,
    fmt::{Display, Formatter},
};

pub(crate) type ExceptionResult<T> = Result<T, Exception>;

#[derive(Debug)]
pub(crate) enum Exception {
    Music21ObjectException(String),
    ChordException(String),
    PitchException(String),
    MicrotoneException(String),
    AccidentalException(String),
}

impl Display for Exception {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for Exception {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}
