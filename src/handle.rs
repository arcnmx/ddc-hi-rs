use {
    crate::{Backend, BackendError, Error},
    ddc::{Ddc, DdcTable, Edid, FeatureCode, TimingMessage, VcpValue},
};

/// A handle allowing communication with a display
#[non_exhaustive]
pub enum Handle {
    #[doc(hidden)]
    #[cfg(feature = "has-ddc-i2c")]
    I2cDevice(ddc_i2c::LinuxDevice),
    #[doc(hidden)]
    #[cfg(feature = "has-ddc-winapi")]
    WinApi(ddc_winapi::Monitor),
    #[doc(hidden)]
    #[cfg(feature = "has-ddc-macos")]
    MacOS(ddc_macos::Monitor),
    #[doc(hidden)]
    #[cfg(feature = "has-nvapi")]
    Nvapi(ddc_i2c::I2cDdc<nvapi::I2c<::std::rc::Rc<nvapi::PhysicalGpu>>>),
}

impl Handle {
    /// Request and parse the display's capabilities string.
    pub fn capabilities(&mut self) -> Result<mccs::Capabilities, Error> {
        mccs_caps::parse_capabilities(&self.capabilities_string()?).map_err(Error::CapabilitiesParseError)
    }

    pub fn backend(&self) -> Backend {
        match self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(..) => Backend::I2cDevice,
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(..) => Backend::WinApi,
            #[cfg(feature = "has-ddc-macos")]
            Handle::MacOS(..) => Backend::MacOS,
            #[cfg(feature = "has-nvapi")]
            Handle::Nvapi(..) => Backend::Nvapi,
        }
    }

    pub fn mccs_version(&mut self) -> Result<Option<mccs::Version>, Error> {
        let version = self.get_vcp_feature(0xdf)?;
        let version = mccs::Version::new(version.sh, version.sl);
        Ok(if version != mccs::Version::default() {
            Some(version)
        } else {
            None
        })
    }

    pub fn edid_data(&mut self) -> Result<Vec<u8>, Error> {
        let len = match self.backend() {
            #[cfg(feature = "has-nvapi")]
            Backend::Nvapi => 0x80,
            _ => 0x100,
        };
        let mut edid = vec![0u8; len];
        let len = self.read_edid(0, &mut edid)?;
        edid.truncate(len);
        Ok(edid)
    }
}

impl ddc::DdcHost for Handle {
    type Error = Error;

    fn sleep(&mut self) {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.sleep(),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => monitor.sleep(),
            #[cfg(feature = "has-ddc-macos")]
            Handle::MacOS(ref mut monitor) => monitor.sleep(),
            #[cfg(feature = "has-nvapi")]
            Handle::Nvapi(ref mut i2c) => i2c.sleep(),
        }
    }
}

impl Edid for Handle {
    type EdidError = Error;

    fn read_edid(&mut self, offset: u8, data: &mut [u8]) -> Result<usize, Self::EdidError> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c
                .read_edid(offset, data)
                .map_err(|e| BackendError::I2cDeviceError(ddc_i2c::Error::I2c(e)).into()),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => Err(Error::UnsupportedOp), // TODO
            #[cfg(feature = "has-ddc-macos")]
            Handle::MacOS(ref mut monitor) => Err(Error::UnsupportedOp), // TODO
            #[cfg(feature = "has-nvapi")]
            Handle::Nvapi(ref mut i2c) => {
                // XXX: hack around broken nvidia drivers
                // the register argument doesn't seem to work at all,
                // so write the edid eeprom offset here first
                i2c.inner_mut().set_address(0x50);
                let _ = i2c.inner_mut().nvapi_write(&[], &[0]);

                i2c.read_edid(offset, data)
                    .map_err(|e| BackendError::NvapiError(ddc_i2c::Error::I2c(e)).into())
            },
        }
    }
}

impl Ddc for Handle {
    fn capabilities_string(&mut self) -> Result<Vec<u8>, Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.capabilities_string().map_err(BackendError::I2cDeviceError),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => monitor.capabilities_string().map_err(BackendError::WinApiError),
            #[cfg(feature = "has-ddc-macos")]
            Handle::MacOS(ref mut monitor) => monitor.capabilities_string().map_err(BackendError::MacOsError),
            #[cfg(feature = "has-nvapi")]
            Handle::Nvapi(ref mut i2c) => i2c.capabilities_string().map_err(BackendError::NvapiError),
        }
        .map_err(Error::CapabilitiesReadError)
    }

    fn get_vcp_feature(&mut self, code: FeatureCode) -> Result<VcpValue, Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.get_vcp_feature(code).map_err(BackendError::I2cDeviceError),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => monitor.get_vcp_feature(code).map_err(BackendError::WinApiError),
            #[cfg(feature = "has-ddc-macos")]
            Handle::MacOS(ref mut monitor) => monitor.get_vcp_feature(code).map_err(BackendError::MacOsError),
            #[cfg(feature = "has-nvapi")]
            Handle::Nvapi(ref mut i2c) => i2c.get_vcp_feature(code).map_err(BackendError::NvapiError),
        }
        .map_err(From::from)
    }

    fn set_vcp_feature(&mut self, code: FeatureCode, value: u16) -> Result<(), Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.set_vcp_feature(code, value).map_err(BackendError::I2cDeviceError),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => monitor.set_vcp_feature(code, value).map_err(BackendError::WinApiError),
            #[cfg(feature = "has-ddc-macos")]
            Handle::MacOS(ref mut monitor) => monitor.set_vcp_feature(code, value).map_err(BackendError::MacOsError),
            #[cfg(feature = "has-nvapi")]
            Handle::Nvapi(ref mut i2c) => i2c.set_vcp_feature(code, value).map_err(BackendError::NvapiError),
        }
        .map_err(From::from)
    }

    fn save_current_settings(&mut self) -> Result<(), Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.save_current_settings().map_err(BackendError::I2cDeviceError),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => monitor.save_current_settings().map_err(BackendError::WinApiError),
            #[cfg(feature = "has-ddc-macos")]
            Handle::MacOS(ref mut monitor) => monitor.save_current_settings().map_err(BackendError::MacOsError),
            #[cfg(feature = "has-nvapi")]
            Handle::Nvapi(ref mut i2c) => i2c.save_current_settings().map_err(BackendError::NvapiError),
        }
        .map_err(From::from)
    }

    fn get_timing_report(&mut self) -> Result<TimingMessage, Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.get_timing_report().map_err(BackendError::I2cDeviceError),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => monitor.get_timing_report().map_err(BackendError::WinApiError),
            #[cfg(feature = "has-ddc-macos")]
            Handle::MacOS(ref mut monitor) => monitor.get_timing_report().map_err(BackendError::MacOsError),
            #[cfg(feature = "has-nvapi")]
            Handle::Nvapi(ref mut i2c) => i2c.get_timing_report().map_err(BackendError::NvapiError),
        }
        .map_err(From::from)
    }
}

impl DdcTable for Handle {
    fn table_read(&mut self, code: FeatureCode) -> Result<Vec<u8>, Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c
                .table_read(code)
                .map_err(|e| Error::LowLevelError(BackendError::I2cDeviceError(e))),
            #[cfg(feature = "has-ddc-macos")]
            Handle::MacOS(ref mut i2c) => i2c
                .table_read(code)
                .map_err(|e| Error::LowLevelError(BackendError::MacOsError(e))),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(_) => Err(Error::UnsupportedOp),
            #[cfg(feature = "has-nvapi")]
            Handle::Nvapi(ref mut i2c) => i2c
                .table_read(code)
                .map_err(|e| Error::LowLevelError(BackendError::NvapiError(e))),
        }
    }

    fn table_write(&mut self, code: FeatureCode, offset: u16, value: &[u8]) -> Result<(), Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c
                .table_write(code, offset, value)
                .map_err(|e| Error::LowLevelError(BackendError::I2cDeviceError(e))),
            #[cfg(feature = "has-ddc-macos")]
            Handle::MacOS(ref mut i2c) => i2c
                .table_write(code, offset, value)
                .map_err(|e| Error::LowLevelError(BackendError::MacOsError(e))),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(_) => Err(Error::UnsupportedOp),
            #[cfg(feature = "has-nvapi")]
            Handle::Nvapi(ref mut i2c) => i2c
                .table_write(code, offset, value)
                .map_err(|e| Error::LowLevelError(BackendError::NvapiError(e))),
        }
    }
}
