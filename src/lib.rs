//#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/ddc-hi/0.5.0")]

//! High level DDC/CI monitor controls.
//!
//! # Example
//!
//! ```rust,no_run
//! use ddc_hi::{Ddc, Display};
//!
//! for mut display in Display::enumerate() {
//!     display.update_capabilities().unwrap();
//!     println!("{:?} {}: {:?} {:?}",
//!         display.info.backend, display.info.id,
//!         display.info.manufacturer_id, display.info.model_name
//!     );
//!     if let Some(feature) = display.info.mccs_database.get(0xdf) {
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

pub use {
    self::{
        backend::Backend,
        display_info::DisplayInfo,
        error::{BackendError, Error},
        handle::Handle,
        query::Query,
    },
    ddc::{Ddc, DdcHost, DdcTable, FeatureCode, TimingMessage, VcpValue, VcpValueType},
};

/// An active handle to a connected display.
pub struct Display {
    /// The inner communication handle used for DDC commands.
    pub handle: Handle,
    /// Information about the connected display.
    pub info: DisplayInfo,
}

impl Display {
    /// Create a new display from the specified handle.
    pub fn new(handle: Handle, info: DisplayInfo) -> Self {
        Display { handle, info }
    }

    /// Updates the display info with data retrieved from the device's
    /// reported capabilities.
    pub fn update_capabilities(&mut self) -> Result<(), Error> {
        let (backend, id) = (self.info.backend, self.info.id.clone());
        let caps = self.handle.capabilities()?;
        let info = DisplayInfo::from_capabilities(backend, id, &caps);
        if info.mccs_version.is_some() {
            self.info.mccs_database = Default::default();
        }
        self.info.update_from(&info);

        Ok(())
    }

    /// Update some display info.
    pub fn update_from_ddc(&mut self) -> Result<(), Error> {
        self.info.update_from_ddc(&mut self.handle)
    }
}
