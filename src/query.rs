use crate::{Backend, DisplayInfo};

/// A query to filter out matching displays.
///
/// Most comparisons must match the full string.
pub enum Query {
    /// Matches any display
    Any,
    /// Matches a display on the given backend
    Backend(Backend),
    /// Matches a display with the specified ID
    Id(String),
    /// Matches a display with the specified manufacturer
    ManufacturerId(String),
    /// Matches a display with the specified model name
    ModelName(String),
    /// Matches a display with the specified serial number
    SerialNumber(String),
    /// At least one of the queries must match
    Or(Vec<Query>),
    /// All of the queries must match
    And(Vec<Query>),
}

impl Query {
    /// Queries whether the provided display info is a match.
    pub fn matches(&self, info: &DisplayInfo) -> bool {
        match *self {
            Query::Any => true,
            Query::Backend(backend) => info.backend == backend,
            Query::Id(ref id) => &info.id == id,
            Query::ManufacturerId(ref id) => info.manufacturer_id.as_ref() == Some(id),
            Query::ModelName(ref model) => info.model_name.as_ref() == Some(model),
            Query::SerialNumber(ref serial) => info.serial_number.as_ref() == Some(serial),
            Query::Or(ref query) => query.iter().any(|q| q.matches(info)),
            Query::And(ref query) => query.iter().all(|q| q.matches(info)),
        }
    }
}
