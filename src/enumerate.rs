use {
    crate::{BackendError, Display, Error, Handle},
    log::warn,
};

impl Display {
    #[cfg(feature = "has-ddc-i2c")]
    pub fn enumerate_i2c() -> std::io::Result<impl Iterator<Item = std::io::Result<Display>>> {
        use std::os::unix::fs::MetadataExt;

        let devs = ddc_i2c::UdevEnumerator::new()?;
        Ok(devs.enumerate().map(|(i, ddc)| {
            ddc.open().map(|ddc| {
                let id = ddc.inner_ref().inner_ref().metadata().map(|meta| meta.rdev());
                Display::new(Handle::I2cDevice(ddc), match id {
                    Ok(dev) => dev.to_string(),
                    Err(..) => format!("index:{i}"),
                })
            })
        }))
    }

    #[cfg(feature = "has-ddc-winapi")]
    pub fn enumerate_winapi() -> Result<impl Iterator<Item = Display>, std::io::Error> {
        let devs = ddc_winapi::Monitor::enumerate()?;
        Ok(devs.into_iter().map(|ddc| {
            let id = ddc.description();
            Display::new(Handle::WinApi(ddc), id)
        }))
    }

    #[cfg(feature = "has-ddc-macos")]
    pub fn enumerate_macos() -> Result<impl Iterator<Item = Display>, ddc_macos::Error> {
        let devs = ddc_macos::Monitor::enumerate()?;
        Ok(devs.into_iter().map(|ddc| {
            let id = ddc.description();
            Display::new(Handle::MacOS(ddc), id)
        }))
    }

    #[cfg(feature = "has-nvapi")]
    pub fn enumerate_nvapi() -> nvapi::Result<impl Iterator<Item = nvapi::Result<Display>>> {
        use std::rc::Rc;

        nvapi::initialize()?;
        Ok(nvapi::PhysicalGpu::enumerate()?.into_iter().flat_map(|gpu| {
            let gpu = Rc::new(gpu);
            let id_prefix = gpu.short_name().unwrap_or("NVAPI".into());
            let (errors, ids) = match gpu.display_ids_connected(nvapi::ConnectedIdsFlags::empty()) {
                Ok(ids) => (None, ids),
                Err(e) => (Some(Err(e)), Default::default()),
            };
            errors.into_iter().chain(ids.into_iter().map(move |id| {
                let mut i2c = nvapi::I2c::new(gpu.clone(), id.display_id);
                // TODO: port=Some(1) instead?
                // docs seem to indicate it's not optional, but the one example I can
                // find keeps it unset so...
                i2c.set_port(None, true);

                let ddc = ddc_i2c::I2cDdc::new(i2c);

                let idstr = format!("displayid:{}/{}", id_prefix, id.display_id);
                Ok(Display::new(Handle::Nvapi(ddc), idstr))
            }))
        }))
    }

    pub fn enumerate_all() -> impl Iterator<Item = Result<Display, BackendError>> {
        fn enumerate_backend<D, E>(displays: Result<D, E>) -> impl Iterator<Item = Result<Display, BackendError>>
        where
            D: IntoIterator<Item = Result<Display, E>>,
            E: Into<BackendError>,
        {
            let (errors, displays) = match displays {
                Ok(displays) => (None, Some(displays.into_iter().map(|r| r.map_err(Into::into)))),
                Err(e) => (Some(Err(e.into())), None),
            };
            displays.into_iter().flat_map(|d| d).chain(errors)
        }

        let displays = std::iter::empty();

        #[cfg(feature = "has-ddc-i2c")]
        let displays = displays.chain(enumerate_backend(
            Self::enumerate_i2c()
                .map(|d| d.map(|d| d.map_err(|e| BackendError::I2cDeviceError(ddc_i2c::Error::I2c(e.into())))))
                .map_err(|e| BackendError::I2cDeviceError(ddc_i2c::Error::I2c(e))),
        ));

        #[cfg(feature = "has-ddc-winapi")]
        let displays = displays.chain(enumerate_backend(
            Self::enumerate_winapi()
                .map(|d| d.map(Ok))
                .map_err(|e| BackendError::WinApiError(e.into())),
        ));

        #[cfg(feature = "has-ddc-macos")]
        let displays = displays.chain(enumerate_backend(
            Self::enumerate_macos()
                .map(|d| d.map(Ok))
                .map_err(|e| BackendError::MacOsError(e.into())),
        ));

        #[cfg(feature = "has-nvapi")]
        let displays = displays.chain(enumerate_backend(
            Self::enumerate_nvapi()
                .map(|d| d.map(|d| d.map_err(|e| BackendError::NvapiError(ddc_i2c::Error::I2c(e.into())))))
                .map_err(|e| BackendError::NvapiError(ddc_i2c::Error::I2c(e.into()))),
        ));

        displays
    }

    /// Enumerate all detected displays.
    pub fn enumerate() -> Vec<Self> {
        Self::enumerate_all()
            .into_iter()
            .map(|display| {
                display.map(|mut display| match display.update_edid() {
                    Ok(()) | Err(Error::UnsupportedOp) => display,
                    Err(e) => {
                        warn!("Failed to read EDID for a {} display: {}", display.backend(), e);
                        display
                    },
                })
            })
            .filter_map(|display| match display {
                Ok(display) => Some(display),
                Err(e) => {
                    warn!("Failed to enumerate a {} display: {}", e.backend(), e);
                    None
                },
            })
            .collect()
    }
}
