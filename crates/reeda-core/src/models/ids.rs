use serde::{Deserialize, Serialize};
use std::fmt;

/// Newtype wrapper for book identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BookId(pub uuid::Uuid);

impl BookId {
    /// Generate a new random `BookId`.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for BookId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BookId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for BookId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(uuid::Uuid::parse_str(s)?))
    }
}

impl TryFrom<&str> for BookId {
    type Error = uuid::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

/// Newtype wrapper for chapter identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChapterId(pub uuid::Uuid);

impl ChapterId {
    /// Generate a new random `ChapterId`.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for ChapterId {
    fn default() -> Self {
        Self::new()
    }
}

/// Newtype wrapper for annotation identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AnnotationId(pub uuid::Uuid);

impl AnnotationId {
    /// Generate a new random `AnnotationId`.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for AnnotationId {
    fn default() -> Self {
        Self::new()
    }
}

/// Newtype wrapper for shelf identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShelfId(pub uuid::Uuid);

impl Default for ShelfId {
    fn default() -> Self {
        Self::new()
    }
}

impl ShelfId {
    /// Generate a new random `ShelfId`.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
