use std::{fmt, str};

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
        write!(f, "{}", match *self {
            Backend::I2cDevice => "i2c-dev",
            Backend::WinApi => "winapi",
            Backend::Nvapi => "nvapi",
            Backend::MacOS => "macos",
        })
    }
}

impl str::FromStr for Backend {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "i2c-dev" => Backend::I2cDevice,
            "winapi" => Backend::WinApi,
            "nvapi" => Backend::Nvapi,
            "macos" => Backend::MacOS,
            _ => return Err(()),
        })
    }
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
}
