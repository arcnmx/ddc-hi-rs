use {
    crate::{Backend, Display, DisplayInfo, Handle},
    ddc::Edid,
    log::warn,
};

impl Display {
    /// Enumerate all detected displays.
    pub fn enumerate() -> Vec<Self> {
        let mut displays = Vec::new();

        #[cfg(feature = "has-ddc-i2c")]
        {
            use std::os::unix::fs::MetadataExt;

            if let Ok(devs) = ddc_i2c::I2cDeviceEnumerator::new() {
                displays.extend(
                    devs.map(|mut ddc| -> Result<_, String> {
                        let id = ddc
                            .inner_ref()
                            .inner_ref()
                            .metadata()
                            .map(|meta| meta.rdev())
                            .unwrap_or(Default::default());
                        let mut edid = vec![0u8; 0x100];
                        ddc.read_edid(0, &mut edid)
                            .map_err(|e| format!("failed to read EDID for i2c-{}: {}", id, e))?;
                        let info = DisplayInfo::from_edid(Backend::I2cDevice, id.to_string(), edid)
                            .map_err(|e| format!("failed to parse EDID for i2c-{}: {}", id, e))?;
                        Ok(Display::new(Handle::I2cDevice(ddc), info))
                    })
                    .filter_map(|d| match d {
                        Ok(v) => Some(v),
                        Err(e) => {
                            warn!("Failed to enumerate a display: {}", e);
                            None
                        },
                    }),
                )
            }
        }

        #[cfg(feature = "has-ddc-winapi")]
        {
            if let Ok(devs) = ddc_winapi::Monitor::enumerate() {
                displays.extend(devs.into_iter().map(|ddc| {
                    let info = DisplayInfo::new(Backend::WinApi, ddc.description());
                    Display::new(Handle::WinApi(ddc), info)
                }))
            }
        }

        #[cfg(feature = "has-ddc-macos")]
        {
            if let Ok(devs) = ddc_macos::Monitor::enumerate() {
                displays.extend(devs.into_iter().map(|ddc| {
                    let info = ddc
                        .edid()
                        .and_then(|edid| DisplayInfo::from_edid(Backend::MacOS, ddc.description(), edid).ok())
                        .unwrap_or(DisplayInfo::new(Backend::MacOS, ddc.description()));
                    Display::new(Handle::MacOS(ddc), info)
                }))
            }
        }

        #[cfg(feature = "has-nvapi")]
        {
            use std::rc::Rc;

            if let Ok(_) = nvapi::initialize() {
                if let Ok(gpus) = nvapi::PhysicalGpu::enumerate() {
                    for gpu in gpus {
                        let gpu = Rc::new(gpu);
                        let id_prefix = gpu.short_name().unwrap_or("NVAPI".into());
                        if let Ok(ids) = gpu.display_ids_connected(nvapi::ConnectedIdsFlags::empty()) {
                            for id in ids {
                                // TODO: it says mask, is it actually `1<<display_id` instead?
                                let mut i2c = nvapi::I2c::new(gpu.clone(), id.display_id);
                                // TODO: port=Some(1) instead? docs seem to indicate it's not optional,
                                // but the one example I can find keeps it unset so...
                                i2c.set_port(None, true);

                                // hack around broken nvidia drivers
                                // the register argument doesn't seem to work at all
                                // so write the edid eeprom offset here first
                                i2c.set_address(0x50);
                                let _ = i2c.nvapi_write(&[], &[0]);

                                let mut ddc = ddc_i2c::I2cDdc::new(i2c);

                                let idstr = format!("{}/{}:{:?}", id_prefix, id.display_id, id.connector);
                                let mut edid = vec![0u8; 0x80]; // 0x100
                                let res = ddc
                                    .read_edid(0, &mut edid)
                                    .map_err(|e| format!("failed to read EDID: {}", e))
                                    .and_then(|_| {
                                        DisplayInfo::from_edid(Backend::Nvapi, idstr, edid)
                                            .map_err(|e| format!("failed to parse EDID: {}", e))
                                    })
                                    .map(|info| Display::new(Handle::Nvapi(ddc), info));
                                match res {
                                    Ok(ddc) => displays.push(ddc),
                                    Err(e) => warn!(
                                        "Failed to enumerate NVAPI display {}/{}:{:?}: {}",
                                        id_prefix, id.display_id, id.connector, e
                                    ),
                                }
                            }
                        }
                    }
                }
            }
        }

        displays
    }
}
