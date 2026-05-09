use std::fmt;
use std::io;

#[derive(Debug)]
pub enum MetadataError {
    Io(io::Error),
    UnsupportedFormat,
    Malformed(&'static str),
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::UnsupportedFormat => write!(f, "Unsupported image format"),
            Self::Malformed(why) => write!(f, "Malformed image data: {}", why),
        }
    }
}

impl std::error::Error for MetadataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for MetadataError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, MetadataError>;
