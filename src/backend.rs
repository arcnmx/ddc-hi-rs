use {
    std::{fmt, str},
    thiserror::Error,
};

/// Identifies the backend driver used to communicate with a display.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Backend {
    /// Linux i2c-dev driver
    I2cDevice,
    /// Windows Monitor Configuration API
    WinApi,
    /// NVIDIA NVAPI driver
    Nvapi,
    /// MacOS APIs
    MacOS,
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl str::FromStr for Backend {
    type Err = BackendParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "i2c-dev" => Backend::I2cDevice,
            "winapi" => Backend::WinApi,
            "nvapi" => Backend::Nvapi,
            "macos" => Backend::MacOS,
            _ => return Err(BackendParseError { str: s.into() }),
        })
    }
}

#[derive(Debug, Error)]
#[error("unknown backend {str}")]
pub struct BackendParseError {
    pub str: String,
}

impl Backend {
    /// Enumerate the possible backends.
    ///
    /// Backends not supported for the current platform will be excluded.
    pub fn values() -> &'static [Backend] {
        &[
            #[cfg(feature = "has-ddc-i2c")]
            Backend::I2cDevice,
            #[cfg(feature = "has-ddc-winapi")]
            Backend::WinApi,
            #[cfg(feature = "has-nvapi")]
            Backend::Nvapi,
            #[cfg(feature = "has-ddc-macos")]
            Backend::MacOS,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Backend::I2cDevice => "i2c-dev",
            Backend::WinApi => "winapi",
            Backend::Nvapi => "nvapi",
            Backend::MacOS => "macos",
        }
    }
}
