use std::result::Result;
use std::{
    error::Error,
    fmt::{Display, Formatter},
};

pub(crate) type ExceptionResult<T> = Result<T, Exception>;

#[derive(Debug)]
pub(crate) enum Exception {
    Music21Object(String),
    Chord(String),
    Pitch(String),
    Microtone(String),
    Accidental(String),
    ChordTables(String),
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
