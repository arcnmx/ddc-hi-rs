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
    pub fn enumerate_winapi() -> Result<impl Iterator<Item = Result<Display, ddc_winapi::Error>>, ddc_winapi::Error> {
        use {
            ddc_winapi::{DeviceInfo, DeviceInfoSet, DevicePropertyKey, DisplayDevice, MonitorDevice, Output},
            std::{
                iter,
                sync::{Arc, RwLock},
            },
        };

        fn warn_result<R>(res: Result<R, ddc_winapi::Error>) -> Option<R> {
            match res {
                Ok(v) => Some(v),
                Err(e) => {
                    warn!("enumeration error: {e:?}");
                    None
                },
            }
        }

        fn find_remove<T, F: FnMut(&T) -> bool>(vec: &mut Vec<T>, mut find: F) -> Option<T> {
            let index = vec.iter().enumerate().find_map(|(i, v)| match find(v) {
                true => Some(i),
                false => None,
            });
            index.map(|i| vec.remove(i))
        }

        fn mondev_matches_info(
            info: &DeviceInfo,
            disp_device: Option<&DisplayDevice>,
            mon: &MonitorDevice<'static>,
        ) -> bool {
            if warn_result(info.matches_device(mon)) == Some(true) {
                return true
            } else if let Some(disp_device) = disp_device {
                if disp_device == mon.display() {
                    let bus = info.get::<u32>(DevicePropertyKey::DEVICE_BUS_NUMBER);
                    todo!("match this up by sort/index, also consider whether it matches {bus:?}? because it didn't match {:?}", mon.id())
                }
            }
            false
        }

        fn mondisp<'a>(mon: Result<&'a MonitorDevice, &'a DisplayDevice>) -> &'a DisplayDevice {
            match mon {
                Ok(mon) => mon.display(),
                Err(dev) => dev,
            }
        }

        fn mon_matches_dispdev(
            disp_device: Option<&DisplayDevice>,
            mon: Option<Result<&MonitorDevice, &DisplayDevice>>,
        ) -> bool {
            let dev = mon.map(mondisp);
            if dev == disp_device {
                true
            } else {
                false
            }
        }

        let outputs = Output::enumerate()?;
        let display_devices: Vec<_> = DisplayDevice::enumerate().collect();
        let mut monitor_devices: Vec<_> = display_devices
            .iter()
            .flat_map(|display| display.enumerate_monitors().map(|mon| mon.owned()))
            .collect();

        let si_monitors = warn_result(DeviceInfoSet::monitors());
        let si_monitors = si_monitors.into_iter().flat_map(|set| set.enumerate_static());
        let si_monitors = si_monitors.filter(|info| {
            info.as_ref()
                .map(|info| warn_result(info.get::<bool>(DevicePropertyKey::DEVICE_IS_PRESENT)) != Some(Some(false)))
                .unwrap_or(true)
        });

        let si_monitors: Vec<_> = si_monitors
            .map(|info| {
                info.map(|info| {
                    let disp_device = display_devices
                        .iter()
                        .find(|dev| warn_result(info.matches_device(dev)) == Some(true));
                    let mon_device =
                        find_remove(&mut monitor_devices, |mon| mondev_matches_info(&info, disp_device, mon));
                    if let Some(mon) = mon_device {
                        debug_assert_eq!(Some(mon.display()), disp_device);
                        (info, Some(Ok(mon)))
                    } else if let Some(dev) = disp_device.cloned() {
                        (info, Some(Err(dev)))
                    } else {
                        (info, None)
                    }
                })
            })
            .collect();
        let si_monitors = Arc::new(RwLock::new(si_monitors));
        let si_monitors_post = si_monitors.clone();

        let monitors = outputs.enumerate().flat_map(move |(i, output)| {
            let oinfo = warn_result(output.info());
            let si_monitors = si_monitors.clone();
            let disp_device = display_devices
                .iter()
                .find(|dev| oinfo.map(|oinfo| oinfo.device_matches_display(dev)).unwrap_or_default());
            if disp_device.is_none() {
                warn!("found no matching display device for {output:?}");
            }
            let disp_device = disp_device.cloned();
            let (phy_err, phys) = match output.enumerate_monitors() {
                Ok(phys) => (None, Some(phys.enumerate())),
                Err(e) => (Some(e), None),
            };
            let phy_err = phy_err.into_iter().map(move |e| {
                let hmon = (Err(e), output, i);
                (hmon, disp_device.map(Err), None)
            });
            let phys = phys.into_iter().flatten().map(move |(ii, physical_monitor)| {
                let si_mon = find_remove(&mut *si_monitors.write().unwrap(), |info| {
                    info.as_ref()
                        .map(|(_info, mon)| {
                            mon_matches_dispdev(disp_device.as_ref(), mon.as_ref().map(|res| res.as_ref()))
                        })
                        .unwrap_or_default()
                });
                let hmon = (Ok((physical_monitor, ii)), output, i);
                match si_mon {
                    Some(Ok((si_mon, mon))) => (hmon, mon, Some(Ok(si_mon))),
                    Some(Err(e)) => (hmon, disp_device.map(Err), Some(Err(e))),
                    None => (hmon, disp_device.map(Err), None),
                }
            });
            phy_err.into_iter().chain(phys)
        });

        let devs = monitors
            .map(|(phy, mon, simon)| (Some(phy), mon, simon))
            .chain(iter::from_fn({
                let mut simons = None;
                move || {
                    let si_monitors = simons.get_or_insert_with(|| {
                        si_monitors_post
                            .write()
                            .unwrap()
                            .drain(..)
                            .collect::<Vec<_>>()
                            .into_iter()
                    });
                    si_monitors.next().map(|info| match info {
                        Ok((info, mon)) => (None, mon, Some(Ok(info))),
                        Err(e) => (None, None, Some(Err(e))),
                    })
                }
            }));

        Ok(devs.enumerate().map(|(i, (monitor, mon_device, monitor_info))| {
            let id = mon_device
                .as_ref()
                .and_then(|mon| match mon {
                    Ok(mon) => Some(format!("mon:{}", mon.id())),
                    _ => None,
                })
                .or_else(|| {
                    monitor_info
                        .as_ref()
                        .and_then(|simon| simon.as_ref().ok())
                        .and_then(|simon| {
                            warn_result(simon.property(DevicePropertyKey::DEVICE_INSTANCE_ID))
                                .flatten()
                                .map(|iid| format!("si:iid:{iid}"))
                                .or_else(|| {
                                    warn_result(simon.property(DevicePropertyKey::DEVICE_HARDWARE_IDS))
                                        .flatten()
                                        .map(|ids| format!("si:hw:{ids}"))
                                })
                        })
                })
                .or_else(|| {
                    monitor.as_ref().and_then(|(phy, hmon, i)| match phy {
                        Ok((phy, ii)) => warn_result(hmon.info())
                            .map(|info| format!("hmon:{}\\Monitor{ii}", info.device_name()))
                            .or_else(|| match &phy.description().to_string()[..] {
                                "Generic PnP Monitor" => None,
                                desc => Some(format!("desc:{}", desc)),
                            })
                            .or_else(|| Some(format!("hmoni:{i}/{ii}"))),
                        Err(_) => None,
                    })
                });
            let id = id.unwrap_or_else(|| format!("index:{i}"));
            let id = id.replace("\\", "/").replace(&['{', '}'], "");

            let monitor = monitor.map(|(phy, ..)| phy.map(|(p, _)| p)).transpose();
            let monitor_info = monitor_info.transpose();
            let (monitor, monitor_info) = match (monitor, monitor_info) {
                (Err(e), Err(_) | Ok(None)) => return Err(e),
                (Ok(None), Err(e)) => return Err(e),
                (Ok(None), Ok(None)) => todo!("no active handles available for {:?}", mon_device),
                (m, info) => (warn_result(m).flatten(), warn_result(info).flatten()),
            };
            debug_assert!(monitor.is_some() || monitor_info.is_some());

            Ok(Display::new(Handle::WinApi { monitor, monitor_info }, id))
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
                .map(|d| d.map(|d| d.map_err(|e| BackendError::WinApiError(e.into()))))
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
