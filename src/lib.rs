//#![deny(missing_docs)]
#![doc(html_root_url = "http://arcnmx.github.io/ddc-hi-rs/")]

//! High level DDC/CI monitor controls.

extern crate ddc;
extern crate edid;
extern crate mccs;
extern crate mccs_caps;
extern crate mccs_db;
extern crate failure;
#[cfg(feature = "has-ddc-i2c")]
extern crate ddc_i2c;
#[cfg(feature = "has-ddc-winapi")]
extern crate ddc_winapi;
#[cfg(feature = "has-nvapi")]
extern crate nvapi;

use std::{io, fmt, str};
use std::iter::FromIterator;
use failure::Error;
use ddc::Edid;

pub use ddc::{Ddc, DdcTable, DdcHost, FeatureCode, VcpValue, VcpValueType, TimingMessage};

/// Identifying information about an attached display.
///
/// Not all information will be available, particularly on backends like
/// WinAPI that do not support EDID.
//#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Clone, Debug)]
pub struct DisplayInfo {
    /// Identifies the backend or driver used to communicate with the display.
    pub backend: Backend,
    /// A unique identifier for the display, format is specific to the backend.
    pub id: String,
    /// A three-character identifier of the manufacturer of the display.
    pub manufacturer_id: Option<String>,
    /// A number that identifies the product model.
    pub model_id: Option<u16>,
    /// The version and revision of the product.
    pub version: Option<(u8, u8)>,
    /// Serial number of the device
    pub serial: Option<u32>,
    /// Year the display was manufactured.
    pub manufacture_year: Option<u8>,
    /// Week the display was manufactured.
    pub manufacture_week: Option<u8>,
    /// The model name of the display.
    pub model_name: Option<String>,
    /// Human-readable serial number of the device.
    pub serial_number: Option<String>,
    /// Raw EDID data provided by the display.
    pub edid_data: Option<Vec<u8>>,
    /// MCCS VCP version code.
    pub mccs_version: Option<mccs::Version>,
    /// MCCS VCP feature information.
    pub mccs_database: mccs_db::Database,
}

impl DisplayInfo {
    pub fn new(backend: Backend, id: String) -> Self {
        DisplayInfo {
            backend: backend,
            id: id,
            manufacturer_id: None,
            model_id: None,
            version: None,
            serial: None,
            manufacture_year: None,
            manufacture_week: None,
            model_name: None,
            serial_number: None,
            edid_data: None,
            mccs_version: None,
            mccs_database: Default::default(),
        }
    }

    /// Creates a new `DisplayInfo` from unparsed EDID data.
    ///
    /// May fail to parse the EDID data.
    pub fn from_edid(backend: Backend, id: String, edid_data: Vec<u8>) -> io::Result<Self> {
        let edid = edid::parse(&edid_data).to_result()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        let mut model_name = None;
        let mut serial_number = None;

        for desc in edid.descriptors {
            match desc {
                edid::Descriptor::SerialNumber(serial) => serial_number = Some(serial),
                edid::Descriptor::ProductName(model) => model_name = Some(model),
                _ => (),
            }
        }

        Ok(DisplayInfo {
            backend: backend,
            id: id,
            edid_data: Some(edid_data),
            manufacturer_id: Some(String::from_iter(edid.header.vendor.iter())),
            model_id: Some(edid.header.product),
            serial: Some(edid.header.serial),
            version: Some((edid.header.version, edid.header.revision)),
            manufacture_year: Some(edid.header.year),
            manufacture_week: Some(edid.header.week),
            model_name: model_name,
            serial_number: serial_number,
            mccs_version: None,
            mccs_database: Default::default(),
        })
    }

    pub fn from_capabilities(backend: Backend, id: String, caps: &mccs::Capabilities) -> Self {
        let edid = if let Some(ref edid) = caps.edid {
            // TODO: return Result here? warn!()?
            Self::from_edid(backend, id.clone(), edid.clone()).map_err(drop)
        } else {
            Err(())
        };

        let mut res = DisplayInfo {
            backend: backend,
            id: id,
            model_name: caps.model.clone(),
            mccs_version: caps.mccs_version.clone(),
            edid_data: caps.edid.clone(),
            // TODO: VDIF
            serial_number: None,
            manufacturer_id: None,
            model_id: None,
            serial: None,
            version: None,
            manufacture_year: None,
            manufacture_week: None,
            mccs_database: Default::default(),
        };

        if let Some(ver) = res.mccs_version.as_ref() {
            res.mccs_database = mccs_db::Database::from_version(ver);
            res.mccs_database.apply_capabilities(caps);
        }

        if let Ok(edid) = edid {
            // TODO: should this be edid.update_from(&res) instead?
            res.update_from(&edid);
        }

        res
    }

    pub fn update_from(&mut self, info: &DisplayInfo) {
        if self.manufacturer_id.is_none() {
            self.manufacturer_id = info.manufacturer_id.clone()
        }

        if self.model_id.is_none() {
            self.model_id = info.model_id.clone()
        }

        if self.version.is_none() {
            self.version = info.version.clone()
        }

        if self.serial.is_none() {
            self.serial = info.serial.clone()
        }

        if self.manufacture_year.is_none() {
            self.manufacture_year  = info.manufacture_year.clone()
        }

        if self.manufacture_week.is_none() {
            self.manufacture_week = info.manufacture_week.clone()
        }

        if self.model_name.is_none() {
            self.model_name = info.model_name.clone()
        }

        if self.serial_number.is_none() {
            self.serial_number = info.serial_number.clone()
        }

        if self.edid_data.is_none() {
            self.edid_data = info.edid_data.clone()
        }

        if self.mccs_version.is_none() {
            self.mccs_version = info.mccs_version.clone()
        }

        if self.mccs_database.get(0xdf).is_none() {
            if info.mccs_version.is_some() {
                self.mccs_version = info.mccs_version.clone()
            }
            self.mccs_database = info.mccs_database.clone()
        }
    }

    pub fn update_from_ddc<D: Ddc>(&mut self, ddc: &mut D) -> Result<(), D::Error> {
        if self.mccs_version.is_none() {
            let version = ddc.get_vcp_feature(0xdf)?;
            let version = mccs::Version::new(version.mh, version.ml);
            if version != mccs::Version::default() {
                self.mccs_version = Some(version);
                self.mccs_database = mccs_db::Database::from_version(&version);
            }
        }

        Ok(())
    }
}

/// Identifies the backend driver used to communicate with a display.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Backend {
    /// Linux i2c-dev driver
    I2cDevice,
    /// Windows Monitor Configuration API
    WinApi,
    /// NVIDIA NVAPI driver
    Nvapi,
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            Backend::I2cDevice => "i2c-dev",
            Backend::WinApi => "winapi",
            Backend::Nvapi => "nvapi",
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
            _ => return Err(()),
        })
    }
}

impl Backend {
    pub fn values() -> &'static [Backend] {
        &[
            #[cfg(feature = "has-ddc-i2c")]
            Backend::I2cDevice,
            #[cfg(feature = "has-ddc-winapi")]
            Backend::WinApi,
            #[cfg(feature = "has-nvapi")]
            Backend::Nvapi,
        ]
    }
}

pub struct Display {
    handle: Handle,
    info: DisplayInfo,
    filled_caps: bool,
}

impl Display {
    /// Create a new display from the specified handle.
    pub fn new(handle: Handle, info: DisplayInfo) -> Self {
        Display {
            handle: handle,
            info: info,
            filled_caps: false,
        }
    }

    /// Enumerate all detected displays.
    pub fn enumerate() -> Vec<Self> {
        let mut displays = Vec::new();

        #[cfg(feature = "has-ddc-i2c")]
        {
            use std::os::unix::fs::MetadataExt;

            if let Ok(devs) = ddc_i2c::I2cDeviceEnumerator::new() {
                displays.extend(devs
                    .map(|mut ddc| -> Result<_, Error> {
                        let mut edid = vec![0u8; 0x100];
                        ddc.read_edid(0, &mut edid)?;
                        let id = ddc.inner_ref().inner_ref().metadata().map(|meta| meta.rdev().to_string()).unwrap_or(Default::default());
                        let info = DisplayInfo::from_edid(Backend::I2cDevice, id, edid)?;
                        Ok(Display::new(
                            Handle::I2cDevice(ddc),
                            info,
                        ))
                    }).filter_map(|d| d.ok())
                )
            }
        }

        #[cfg(feature = "has-ddc-winapi")]
        {
            if let Ok(devs) = ddc_winapi::Monitor::enumerate() {
                displays.extend(devs.into_iter()
                    .map(|ddc| {
                        let info = DisplayInfo::new(Backend::WinApi, ddc.description());
                        Display::new(
                            Handle::WinApi(ddc),
                            info,
                        )
                    })
                )
            }
        }

        #[cfg(feature = "has-nvapi")]
        {
            if let Ok(_) = nvapi::initialize() {
                // TODO: this
            }
        }

        displays
    }

    /// Retrieve the inner communication handle, which can be used for DDC
    /// commands.
    pub fn handle(&mut self) -> &mut Handle {
        &mut self.handle
    }

    /// Information about the connected display.
    pub fn info(&self) -> &DisplayInfo {
        &self.info
    }

    /// Updates the display info with data retrieved from the device's
    /// reported capabilities.
    pub fn update_capabilities(&mut self) -> Result<(), Error> {
        if !self.filled_caps {
            let (backend, id) = (self.info.backend, self.info.id.clone());
            let caps = self.handle.capabilities()?;
            let info = DisplayInfo::from_capabilities(
                backend, id,
                &caps,
            );
            if info.mccs_version.is_some() {
                self.info.mccs_database = Default::default();
            }
            self.info.update_from(&info);
        }

        Ok(())
    }

    /// Update some display info.
    pub fn update_from_ddc(&mut self) -> Result<(), Error> {
        self.info.update_from_ddc(&mut self.handle)
    }
}

/// A handle allowing communication with a display
pub enum Handle {
    #[doc(hidden)]
    #[cfg(feature = "has-ddc-i2c")]
    I2cDevice(ddc_i2c::I2cDeviceDdc),
    #[doc(hidden)]
    #[cfg(feature = "has-ddc-winapi")]
    WinApi(ddc_winapi::Monitor),
    // TODO: Nvapi
}

impl Handle {
    pub fn capabilities(&mut self) -> Result<mccs::Capabilities, Error> {
        mccs_caps::parse_capabilities(&self.capabilities_string()?)
            .map_err(From::from)
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
        }
    }
}

impl Ddc for Handle {
    fn capabilities_string(&mut self) -> Result<Vec<u8>, Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.capabilities_string().map_err(From::from),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => monitor.capabilities_string().map_err(From::from),
        }
    }

    fn get_vcp_feature(&mut self, code: FeatureCode) -> Result<VcpValue, Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.get_vcp_feature(code).map_err(From::from),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => monitor.get_vcp_feature(code).map_err(From::from),
        }
    }

    fn set_vcp_feature(&mut self, code: FeatureCode, value: u16) -> Result<(), Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.set_vcp_feature(code, value).map_err(From::from),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => monitor.set_vcp_feature(code, value).map_err(From::from),
        }
    }

    fn save_current_settings(&mut self) -> Result<(), Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.save_current_settings().map_err(From::from),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => monitor.save_current_settings().map_err(From::from),
        }
    }

    fn get_timing_report(&mut self) -> Result<TimingMessage, Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.get_timing_report().map_err(From::from),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) => monitor.get_timing_report().map_err(From::from),
        }
    }
}

impl DdcTable for Handle {
    fn table_read(&mut self, code: FeatureCode) -> Result<Vec<u8>, Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.table_read(code).map_err(From::from),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) =>
                Err(io::Error::new(io::ErrorKind::Other, "winapi does not support DDC tables").into()),
        }
    }

    fn table_write(&mut self, code: FeatureCode, offset: u16, value: &[u8]) -> Result<(), Self::Error> {
        match *self {
            #[cfg(feature = "has-ddc-i2c")]
            Handle::I2cDevice(ref mut i2c) => i2c.table_write(code, offset, value).map_err(From::from),
            #[cfg(feature = "has-ddc-winapi")]
            Handle::WinApi(ref mut monitor) =>
                Err(io::Error::new(io::ErrorKind::Other, "winapi does not support DDC tables").into()),
        }
    }
}
