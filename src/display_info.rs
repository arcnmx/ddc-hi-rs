use {
    crate::Backend,
    ddc::Ddc,
    log::{trace, warn},
    std::{fmt, io, iter::FromIterator},
};

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

impl fmt::Display for DisplayInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.backend, self.id)?;

        if let Some(s) = &self.manufacturer_id {
            write!(f, " {}", s)?;
        }

        if let Some(s) = &self.model_name {
            write!(f, " {}", s)?;
        } else if let Some(s) = &self.model_id {
            write!(f, " {}", s)?;
        }

        Ok(())
    }
}

impl DisplayInfo {
    /// Create an empty `DisplayInfo`.
    pub fn new(backend: Backend, id: String) -> Self {
        DisplayInfo {
            backend,
            id,
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
        trace!("DisplayInfo::from_edid({:?}, {})", backend, id);

        let edid = edid::parse(&edid_data)
            .to_result()
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
            backend,
            id,
            edid_data: Some(edid_data),
            manufacturer_id: Some(String::from_iter(edid.header.vendor.iter())),
            model_id: Some(edid.header.product),
            serial: Some(edid.header.serial),
            version: Some((edid.header.version, edid.header.revision)),
            manufacture_year: Some(edid.header.year),
            manufacture_week: Some(edid.header.week),
            model_name,
            serial_number,
            mccs_version: None,
            mccs_database: Default::default(),
        })
    }

    /// Create a new `DisplayInfo` from parsed capabilities.
    pub fn from_capabilities(backend: Backend, id: String, caps: &mccs::Capabilities) -> Self {
        trace!("DisplayInfo::from_capabilities({:?}, {})", backend, id);

        let edid = caps
            .edid
            .clone()
            .map(|edid| Self::from_edid(backend, id.clone(), edid))
            .transpose();

        let mut res = DisplayInfo {
            backend,
            id,
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

        match edid {
            Ok(Some(edid)) => {
                // TODO: should this be edid.update_from(&res) instead?
                res.update_from(&edid);
            },
            Ok(None) => (),
            Err(e) => {
                warn!("Failed to parse edid from caps of {}: {}", res, e);
            },
        }

        res
    }

    /// Merge in any missing information from another `DisplayInfo`
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
            self.manufacture_year = info.manufacture_year.clone()
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

    /// Populate information from a DDC connection.
    ///
    /// This will read the VCP Version (`0xdf`) and fill in the `mccs_database`.
    /// This data will be incomplete compared to being filled in from a capability
    /// string.
    pub fn update_from_ddc<D: Ddc>(&mut self, ddc: &mut D) -> Result<(), D::Error> {
        if self.mccs_version.is_none() {
            trace!("DisplayInfo::update_from_ddc");

            let version = ddc.get_vcp_feature(0xdf)?;
            let version = mccs::Version::new(version.sh, version.sl);
            if version != mccs::Version::default() {
                self.mccs_version = Some(version);
                self.mccs_database = mccs_db::Database::from_version(&version);
            }
        }

        Ok(())
    }
}
