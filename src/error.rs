use std::{fmt::Display, io};

#[derive(Debug)]
#[allow(dead_code)]
pub enum Error {
    NBTError(nbt::Error),
    IOError(io::Error),
    SerdeJsonError(serde_json::Error),
    CSVError(csv::Error),
    NBTMissingField(&'static str),
    IncorrecFlags,
    LOLError,
    NOTLOLError,
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self {
            Self::NBTMissingField(_) => None,
            Self::IncorrecFlags => None,
            Self::LOLError => None,
            Self::NOTLOLError => None,
            &error => error.source(),
        }
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::NBTMissingField(missing_field) => {
                write!(
                    f,
                    "NBT data file is missing the required field \"{}\"",
                    missing_field
                )
            }
            Self::IncorrecFlags => write!(f, "IncorrecFlags"),
            Self::LOLError => write!(f, "LOLError"),
            Self::NOTLOLError => write!(f, "NOTLOLError"),
            &error => {
                write!(f, "{}", error.to_string())
            }
        }
    }
}

impl From<nbt::Error> for Error {
    fn from(value: nbt::Error) -> Self {
        Self::NBTError(value)
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::IOError(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::SerdeJsonError(value)
    }
}

impl From<csv::Error> for Error {
    fn from(value: csv::Error) -> Self {
        Self::CSVError(value)
    }
}
