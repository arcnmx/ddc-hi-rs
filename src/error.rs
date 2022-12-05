use {crate::Backend, std::io, thiserror::Error};

/// The error type for high level DDC/CI monitor operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Unsupported operation.
    #[error("the backend does not support the operation")]
    UnsupportedOp,

    /// An error occurred while parsing EDID data.
    #[error("failed to parse EDID: {0}")]
    EdidParseError(io::Error),

    /// An error occurred while reading the capabilities.
    #[error("failed to read capabilities string: {0}")]
    CapabilitiesReadError(BackendError),

    /// An error occurred while parsing MCCS capabilities.
    #[error("failed to parse MCCS capabilities: {0}")]
    CapabilitiesParseError(io::Error),

    /// Low level errors.
    #[error("low level error: {0}")]
    LowLevelError(#[from] BackendError),
}

/// A wrapper for the DDC backend errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BackendError {
    #[cfg(feature = "has-ddc-i2c")]
    /// I2c error.
    #[error("i2c error: {0}")]
    I2cDeviceError(ddc_i2c::Error<io::Error>),

    #[cfg(feature = "has-ddc-winapi")]
    /// Windows API error.
    #[error("winapi error: {0}")]
    WinApiError(<ddc_winapi::Monitor as ddc::DdcHost>::Error),

    #[cfg(feature = "has-ddc-macos")]
    /// MacOS API error.
    #[error("macOS API error: {0}")]
    MacOsError(<ddc_macos::Monitor as ddc::DdcHost>::Error),

    // NOTE: We use ddc-i2c instead of has-... because the latter actually means
    // ddc-i2c enabled on a Unix platform.
    #[cfg(all(feature = "has-nvapi", feature = "ddc-i2c"))]
    /// Nvapi error.
    #[error("nvapi error: {0}")]
    NvapiError(ddc_i2c::Error<nvapi::Status>),
}

impl BackendError {
    pub fn backend(&self) -> Backend {
        match self {
            #[cfg(feature = "has-ddc-i2c")]
            BackendError::I2cDeviceError(..) => Backend::I2cDevice,
            #[cfg(feature = "has-ddc-winapi")]
            BackendError::WinApiError(..) => Backend::WinApi,
            #[cfg(feature = "has-ddc-macos")]
            BackendError::MacOsError(..) => Backend::MacOS,
            #[cfg(feature = "has-nvapi")]
            BackendError::NvapiError(..) => Backend::Nvapi,
        }
    }
}

impl Error {
    pub fn backend(&self) -> Option<Backend> {
        match self {
            Error::CapabilitiesReadError(e) | Error::LowLevelError(e) => Some(e.backend()),
            _ => None,
        }
    }

    pub fn unsupported_ok<T>(res: Result<T, Self>) -> Result<Option<T>, Self> {
        match res {
            Err(Error::UnsupportedOp) => Ok(None),
            res => res.map(Some),
        }
    }
}
