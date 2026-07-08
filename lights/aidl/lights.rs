/*
 * Copyright (C) 2023 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
//! This module implements the ILights AIDL interface.

use std::collections::HashMap;
use std::sync::Mutex;
use log::{debug, error, info, warn};

use android_hardware_light::aidl::android::hardware::light::{
	HwLight::HwLight, HwLightState::HwLightState, ILights::ILights, LightType::LightType,
};

use binder::{ExceptionCode, Interface, Status};

struct BacklightDevice {
    name:                  &'static str,
    brightness_path:       &'static str,
    max_brightness_path:   &'static str,
    /// Fallback if max_brightness sysfs read fails
    brightness_default_max: u32,
    /// Lowest value the panel displays visibly; used to remap the brightness curve
    brightness_min_visible: u32,
}

const BACKLIGHT_DEVICES: &[BacklightDevice] = &[
    BacklightDevice {
        name:                   "Intel video backlight control",
        brightness_path:        "/sys/class/backlight/intel_backlight/brightness",
        max_brightness_path:    "/sys/class/backlight/intel_backlight/max_brightness",
        brightness_default_max: 4648,
        brightness_min_visible: 20,
    },
    BacklightDevice {
        name:                   "ACPI video backlight control",
        brightness_path:        "/sys/class/backlight/acpi_video0/brightness",
        max_brightness_path:    "/sys/class/backlight/acpi_video0/max_brightness",
        brightness_default_max: 15,
        brightness_min_visible: 1,
    },
    BacklightDevice {
        name:                   "LED video backlight control",
        brightness_path:        "/sys/class/leds/lcd-backlight/brightness",
        max_brightness_path:    "/sys/class/leds/lcd-backlight/max_brightness",
        brightness_default_max: 255,
        brightness_min_visible: 3,
    },
    BacklightDevice {
        name:                   "LCD video backlight control",
        brightness_path:        "/sys/class/backlight/lcd-backlight/brightness",
        max_brightness_path:    "/sys/class/backlight/lcd-backlight/max_brightness",
        brightness_default_max: 255,
        brightness_min_visible: 3,
    },
];

struct Light {
	hw_light: HwLight,
	state: HwLightState,
}

/// Defined so we can implement the ILights AIDL interface.
pub struct LightsService {
	lights: Mutex<HashMap<i32, Light>>,
	backlight_dev: Option<&'static BacklightDevice>,
	max_brightness: u32,
	last_backlight_sysfs: Mutex<Option<u32>>,
}

impl Interface for LightsService {}

impl LightsService {
	fn new(hw_lights: impl IntoIterator<Item = HwLight>) -> Self {
		let mut lights_map = HashMap::new();

		for hw_light in hw_lights {
			lights_map.insert(hw_light.id, Light { hw_light, state: Default::default() });
		}

		let backlight_dev = Self::determine_backlight_device();
		let max_brightness = backlight_dev.map(Self::read_max_brightness).unwrap_or(255);
		
		Self { 
		  lights: Mutex::new(lights_map),
		  backlight_dev,
		  max_brightness,
		  last_backlight_sysfs: Mutex::new(None),
		}
	}
	fn determine_backlight_device() -> Option<&'static BacklightDevice> {
		for dev in BACKLIGHT_DEVICES {
			if std::fs::OpenOptions::new().write(true).open(dev.brightness_path).is_err() {
				continue; // brightness not writable
			}
			if std::fs::File::open(dev.max_brightness_path).is_err() {
				continue; // max_brightness not readable
			}
			info!("Selected backlight device: {}", dev.name);
			return Some(dev);
		}
		warn!("Cannot find any supported backlight controls");
		None
	}

	fn read_max_brightness(dev: &BacklightDevice) -> u32 {	
		match std::fs::read_to_string(dev.max_brightness_path) {
			Ok(content) => match content.trim().parse::<u32>() {
				Ok(val) => {
					info!("Backlight max_brightness = {}", val);
					val
				}
				Err(e) => {
					warn!("Failed to parse max_brightness, using device default {}: {}",dev.brightness_default_max, e);
					dev.brightness_default_max
				}
			},
			Err(e) => {
				warn!("Failed to read max_brightness from {}, using device default {}: {}",dev.max_brightness_path, dev.brightness_default_max, e);
				dev.brightness_default_max
			}
		}
	}
	
	//this conversion is based on google's recommendation in lights.h
	fn color_to_brightness(color: u32) -> u32 {
		((77 * ((color >> 16) & 0x00ff)) + (150 * ((color >> 8) & 0x00ff)) + (29 * (color & 0x00ff))) >> 8
	}
	
	fn map_255_to_sysfs(brightness_255: u32, max_brightness: u32, min_visible: u32,) -> Result<u32, Status> {
		if max_brightness <= min_visible {
			error!("Invalid max_brightness {} <= brightness_min_visible {}",max_brightness, min_visible);
			return Err(Status::new_service_specific_error(-22 /* EINVAL */, None));
		}
		
		if brightness_255 == 0 {
			return Ok(0);
		}
		
		// Use u64 intermediates to avoid overflow on multiplication.
		let span: u64 = (max_brightness - min_visible) as u64;
        	let b255: u64 = brightness_255 as u64;
        	let scaled: u64 = (b255 * span) / 255u64 + (min_visible as u64);
		
		// Clamp to [min_visible, max_brightness]
		let clamped = scaled.min(max_brightness as u64).max(min_visible as u64) as u32;
		Ok(clamped)
	}
	
	fn write_backlight(&self, state: &HwLightState) -> binder::Result<()> {
		let dev = self.backlight_dev.ok_or_else(|| {
			error!("No backlight device available");
			Status::new_exception(ExceptionCode::UNSUPPORTED_OPERATION, None)
		})?;

		let color = state.color as u32;
		let brightness_255 = Self::color_to_brightness(color);
		let brightness_sysfs = Self::map_255_to_sysfs(brightness_255, self.max_brightness, dev.brightness_min_visible)?;
		
		// Skip redundant writes only if we KNOW hardware is already at this value.
		let mut last = self.last_backlight_sysfs.lock().map_err(|_| Status::new_service_specific_error(-5 /* EIO */, None))?;
		if last.is_some_and(|v| v == brightness_sysfs) {
			debug!("Backlight unchanged ({}), skipping sysfs write", brightness_sysfs);
			return Ok(());
		}
		debug!(
          		"Writing backlight: color={:08x} brightness_255={} -> sysfs={}/{}",
            		state.color,
            		brightness_255,
            		brightness_sysfs,
            		self.max_brightness
        	);
		
		// Many sysfs nodes are fine without newline, but newline is safest/most compatible.
		let payload = format!("{brightness_sysfs}\n");
		
		std::fs::write(dev.brightness_path, &payload).map_err(|e| {
			error!("Failed to write backlight to {}: {}", dev.brightness_path, e);
			Status::new_service_specific_error(
				-e.raw_os_error().unwrap_or(5 /* EIO */),
				None,
			)
		})?;
		
		// Update cache ONLY after successful hardware write.
		*last = Some(brightness_sysfs);
		Ok(())
	}
}

impl Default for LightsService {
	fn default() -> Self {
		// Only advertise the BACKLIGHT light when a physical sysfs device is
		// available.  If no device is found the light list is empty, which is
		// valid for a virtual device (CaaS VM) and satisfies the VTS HAL
		// contract: setLightState must succeed for every ID returned by
		// getLights(), so we must not advertise lights we cannot service.
		let hw_lights: Vec<HwLight> = if Self::determine_backlight_device().is_some() {
			vec![HwLight { id: 1, ordinal: 1, r#type: LightType::BACKLIGHT }]
		} else {
			vec![]
		};
		Self::new(hw_lights)
	}
}

impl ILights for LightsService {
	fn setLightState(&self, id: i32, state: &HwLightState) -> binder::Result<()> {
		debug!("Lights setting state for id={} to color {:x}", id, state.color);

		let mut lights = self.lights.lock().unwrap_or_else(|e| e.into_inner());
		if let Some(light) = lights.get_mut(&id) {
			light.state = *state;
			let light_type = light.hw_light.r#type;
			drop(lights);
			if light_type == LightType::BACKLIGHT {
				self.write_backlight(state)?;
			}
			Ok(())
		} else {
			Err(Status::new_exception(ExceptionCode::UNSUPPORTED_OPERATION, None))
		}
	}

	fn getLights(&self) -> binder::Result<Vec<HwLight>> {
		debug!("Lights reporting supported lights");
		Ok(self.lights.lock().unwrap().values().map(|light| light.hw_light).collect())
	}
}
