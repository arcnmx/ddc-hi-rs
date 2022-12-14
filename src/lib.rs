//#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/ddc-hi/0.5.0")]

//! High level DDC/CI monitor controls.
//!
//! # Example
//!
//! ```rust,no_run
//! use ddc_hi::Display;
//! use ddc_hi::traits::*;
//!
//! for mut display in Display::enumerate() {
//!     display.update_all().unwrap();
//!     let info = display.info();
//!     println!("{:?} {}: {:?} {:?}",
//!         info.backend, info.id,
//!         info.manufacturer_id, info.model_name
//!     );
//!     if let Some(feature) = info.mccs_database.get(0xdf) {
//!         let value = display.handle.get_vcp_feature(feature.code).unwrap();
//!         println!("{}: {:?}", feature.name.as_ref().unwrap(), value);
//!     }
//! }
//! ```

mod backend;
mod display_info;
mod enumerate;
mod error;
mod handle;
mod query;

pub use self::{
    backend::Backend,
    display_info::DisplayInfo,
    error::{BackendError, Error},
    handle::Handle,
    query::Query,
};

pub mod traits {
    pub use ddc::{Ddc, DdcHost, DdcTable, Edid};
}
pub use ddc::{FeatureCode, TimingMessage, VcpValue, VcpValueType};
use {log::warn, mccs::Capabilities, std::io};

/// An active handle to a connected display.
#[derive(Debug)]
pub struct Display {
    /// A unique identifier for the display, format is specific to the backend.
    pub id: String,
    /// The inner communication handle used for DDC commands.
    pub handle: Handle,
    /// Capability information about the connected display.
    pub capabilities: Option<Capabilities>,
    /// EDID data exposed by the connected display.
    pub edid_data: Option<Vec<u8>>,
    pub mccs_version: Option<mccs::Version>,
}

impl Display {
    /// Create a new display from the specified handle.
    pub fn new(handle: Handle, id: String) -> Self {
        Display {
            handle,
            id,
            capabilities: None,
            edid_data: None,
            mccs_version: None,
        }
    }

    pub fn backend(&self) -> Backend {
        self.handle.backend()
    }

    /// Information about the connected display.
    pub fn info(&self) -> DisplayInfo {
        let backend = self.backend();
        let mut info = match self.edid_info() {
            Some(Ok(info)) => info,
            e => {
                if let Some(Err(e)) = e {
                    warn!("Failed to parse {}/{} EDID: {:?}", backend, self.id, e);
                }
                DisplayInfo::new(backend, self.id.clone())
            },
        };
        if let Some(caps_info) = self.capabilities_info() {
            if caps_info.mccs_version.is_some() {
                info.mccs_database = Default::default();
            }
            info.update_from(&caps_info);
        }
        if let Some(mccs_version) = self.mccs_version {
            info.mccs_version = Some(mccs_version);
        }
        info
    }

    pub fn mccs_version(&self) -> Option<mccs::Version> {
        match &self.capabilities {
            Some(caps) => caps.mccs_version,
            None => self.mccs_version,
        }
    }

    pub fn mccs_database(&self) -> Option<mccs_db::Database> {
        let mut mccs_database = mccs_db::Database::from_version(&self.mccs_version()?);
        if let Some(caps) = &self.capabilities {
            mccs_database.apply_capabilities(caps);
        }
        Some(mccs_database)
    }

    pub fn capabilities_info(&self) -> Option<DisplayInfo> {
        self.capabilities
            .as_ref()
            .map(|caps| DisplayInfo::from_capabilities(self.backend(), self.id.clone(), caps))
    }

    pub fn edid_info(&self) -> Option<Result<DisplayInfo, io::Error>> {
        let edid = self.edid_data.clone()?;
        Some(DisplayInfo::from_edid(self.backend(), self.id.clone(), edid))
    }

    pub fn update_fast(&mut self, update_caps: bool) -> Result<(), Error> {
        let res_edid = self.update_edid();

        let res_caps = if update_caps || self.edid_data.is_none() {
            self.update_capabilities()
        } else {
            Ok(())
        };

        let caps_version = self.capabilities.as_ref().and_then(|caps| caps.mccs_version);
        let res_version = if caps_version.is_none() {
            self.update_version()
        } else {
            Ok(())
        };

        res_edid.and_then(|()| res_caps).and_then(|()| res_version)
    }

    pub fn update_all(&mut self) -> Result<(), Error> {
        let res_edid = self.update_edid();

        let res_caps = self.update_capabilities();

        let res_version = self.update_version();

        res_edid.and_then(|()| res_caps).and_then(|()| res_version)
    }

    pub fn update_edid(&mut self) -> Result<(), Error> {
        if self.edid_data.is_none() {
            if let Some(edid) = Error::unsupported_ok(self.handle.edid_data())? {
                self.edid_data = Some(edid)
            }
        }
        Ok(())
    }

    /// Updates the display info with data retrieved from the device's
    /// reported capabilities.
    pub fn update_capabilities(&mut self) -> Result<(), Error> {
        if self.capabilities.is_none() {
            if let Some(caps) = Error::unsupported_ok(self.handle.capabilities())? {
                self.capabilities = Some(caps)
            }
        }
        Ok(())
    }

    /// Update some display info.
    pub fn update_version(&mut self) -> Result<(), Error> {
        if self.mccs_version.is_none() {
            if let Some(version) = self.handle.mccs_version()? {
                self.mccs_version = Some(version);
            }
        }
        Ok(())
    }
}
