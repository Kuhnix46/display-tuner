use std::mem;

use anyhow::Result;
use tracing::{debug, info};
use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
    DISPLAYCONFIG_DEVICE_INFO_TYPE, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE,
    DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    DisplayConfigSetDeviceInfo, GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS,
    QueryDisplayConfig, SDC_APPLY, SDC_USE_SUPPLIED_DISPLAY_CONFIG, SetDisplayConfig,
};

pub const DPI_VALUES: [i32; 12] = [100, 125, 150, 175, 200, 225, 250, 300, 350, 400, 450, 500];

#[derive(Default)]
pub struct DisplayTuner {
    pub displays: Vec<DisplayInfo>,
}

#[derive(Debug, Clone)]
pub struct DisplayInfo {
    friendly_name: String,
    source_id: u32,
    width: u32,
    height: u32,
    scaling_current: i32,
    scaling_recommended: i32,
}

#[derive(Debug, Clone)]
pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub scaling: i32,
}

#[repr(C)]
struct DpiScaleGet {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    min_scale_rel: i32,
    cur_scale_rel: i32,
    max_scale_rel: i32,
}

#[repr(C)]
struct DpiScaleSet {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    scale_rel: i32,
}

impl DisplayTuner {
    pub fn enumerate_displays(&mut self) -> Result<Vec<DisplayInfo>> {
        let mut displays = Vec::new();

        let (paths, modes) = self.get_display_config()?;

        for path in &paths {
            debug!("Processing path...");

            let source_mode_idx;
            unsafe {
                source_mode_idx = path.sourceInfo.Anonymous.modeInfoIdx as usize;
            }

            if source_mode_idx == 0xFFFF_FFFF || source_mode_idx >= modes.len() {
                debug!("Skipping invalid mode index: {}", source_mode_idx);
                continue;
            }

            let mode = &modes[source_mode_idx];
            if mode.infoType != DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE {
                debug!("Skipping non-source mode");
                continue;
            }

            let (width, height);
            unsafe {
                width = mode.Anonymous.sourceMode.width;
                height = mode.Anonymous.sourceMode.height;
            }

            let friendly_name = Self::get_display_name_from_path(path)?;
            let scaling = Self::get_display_scaling_from_path(path)?;

            let disp = DisplayInfo {
                friendly_name,
                source_id: path.sourceInfo.id,
                width,
                height,
                scaling_current: scaling.0,
                scaling_recommended: scaling.1,
            };
            info!("{:?}", disp);
            displays.push(disp);
        }

        self.displays.clone_from(&displays);

        Ok(displays)
    }

    pub fn apply_display_config(
        &self,
        display: &DisplayInfo,
        config: &DisplayConfig,
    ) -> Result<()> {
        let resolution_changed = display.width != config.width || display.height != config.height;
        let scaling_changed = display.scaling_current != config.scaling;

        if !resolution_changed && !scaling_changed {
            debug!("Display configuration already matches target, skipping");
            return Ok(());
        }

        if resolution_changed {
            self.apply_display_resolution(display, config)?;
        }

        if scaling_changed {
            self.apply_display_scaling(display, config)?;
        }

        Ok(())
    }

    fn get_display_config(
        &self,
    ) -> Result<(Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>)> {
        unsafe {
            let mut path_count = 0u32;
            let mut mode_count = 0u32;

            let result = GetDisplayConfigBufferSizes(
                QDC_ONLY_ACTIVE_PATHS,
                &raw mut path_count,
                &raw mut mode_count,
            );
            if result.0 != 0 {
                anyhow::bail!("Failed to get display config buffer sizes: {}", result.0);
            }

            let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];

            let result = QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &raw mut path_count,
                paths.as_mut_ptr(),
                &raw mut mode_count,
                modes.as_mut_ptr(),
                None,
            );
            if result.0 != 0 {
                anyhow::bail!("Failed to query display config: {}", result.0);
            }

            debug!("Retrieved {} paths and {} modes", path_count, mode_count);

            Ok((paths, modes))
        }
    }

    fn get_display_name_from_path(path: &DISPLAYCONFIG_PATH_INFO) -> Result<String> {
        let mut target_name = DISPLAYCONFIG_TARGET_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                size: mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                adapterId: path.targetInfo.adapterId,
                id: path.targetInfo.id,
            },
            flags: Default::default(),
            outputTechnology: Default::default(),
            edidManufactureId: 0,
            edidProductCodeId: 0,
            connectorInstance: 0,
            monitorFriendlyDeviceName: [0; 64],
            monitorDevicePath: [0; 128],
        };

        let result;
        unsafe {
            result = DisplayConfigGetDeviceInfo(&raw mut target_name.header);
        }

        if result == 0 {
            let friendly_name = String::from_utf16_lossy(&target_name.monitorFriendlyDeviceName)
                .trim_end_matches('\0')
                .to_string();

            Ok(format!("{} ({})", path.sourceInfo.id, friendly_name))
        } else {
            anyhow::bail!("Failed to get monitor friendly name: {}", result);
        }
    }

    fn get_display_scaling_from_path(path: &DISPLAYCONFIG_PATH_INFO) -> Result<(i32, i32)> {
        let mut dpi_info = DpiScaleGet {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_TYPE(-3i32),
                size: mem::size_of::<DpiScaleGet>() as u32,
                adapterId: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
            },
            min_scale_rel: 0,
            cur_scale_rel: 0,
            max_scale_rel: 0,
        };

        unsafe {
            let result = DisplayConfigGetDeviceInfo(&raw mut dpi_info.header);
            if result != 0 {
                anyhow::bail!("Failed to get DPI info: {result}");
            }
        }

        let min_abs = dpi_info.min_scale_rel.unsigned_abs() as usize;

        let cur_index = min_abs.wrapping_add(dpi_info.cur_scale_rel as usize);
        let rec_index = cur_index - dpi_info.cur_scale_rel as usize;

        if cur_index < DPI_VALUES.len() {
            Ok((DPI_VALUES[cur_index], DPI_VALUES[rec_index]))
        } else {
            anyhow::bail!("DPI index out of range");
        }
    }

    fn apply_display_resolution(
        &self,
        display: &DisplayInfo,
        config: &DisplayConfig,
    ) -> Result<()> {
        let old_width = display.width;
        let old_height = display.height;
        let new_width = config.width;
        let new_height = config.height;
        info!(
            old_width,
            old_height, new_width, new_height, "Changing resolution"
        );

        let (paths, mut modes) = self.get_display_config()?;

        unsafe {
            let path = paths
                .iter()
                .find(|path| path.sourceInfo.id == display.source_id)
                .unwrap();

            let mode_idx = path.sourceInfo.Anonymous.modeInfoIdx as usize;
            let mode = &mut modes[mode_idx];

            mode.Anonymous.sourceMode.width = config.width;
            mode.Anonymous.sourceMode.height = config.height;

            let result = SetDisplayConfig(
                Some(&paths),
                Some(&modes),
                SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG,
            );

            if result != 0 {
                anyhow::bail!("Failed to set display configuration: {}", result);
            }

            info!("Resolution changed successfully");
        }

        Ok(())
    }

    fn apply_display_scaling(&self, display: &DisplayInfo, config: &DisplayConfig) -> Result<()> {
        let old_scaling = display.scaling_current;
        let new_scaling = config.scaling;
        info!(old_scaling, new_scaling, "Changing DPI scaling");

        let (paths, mut _modes) = self.get_display_config()?;
        let path = paths
            .iter()
            .find(|path| path.sourceInfo.id == display.source_id)
            .unwrap();

        let current_scale = Self::get_display_scaling_from_path(path)?;
        let recommoned_scale = current_scale.1;

        let recommoned_scale_idx = DPI_VALUES
            .iter()
            .position(|&v| v == recommoned_scale)
            .unwrap() as i32;

        let target_scale_idx = DPI_VALUES
            .iter()
            .position(|&v| v == config.scaling)
            .unwrap() as i32;

        unsafe {
            let mut dpi_set = DpiScaleSet {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_TYPE(-4i32),
                    size: mem::size_of::<DpiScaleSet>() as u32,
                    adapterId: path.sourceInfo.adapterId,
                    id: display.source_id,
                },
                scale_rel: target_scale_idx - recommoned_scale_idx,
            };

            let result = DisplayConfigSetDeviceInfo(&raw mut dpi_set.header);
            if result != 0 {
                anyhow::bail!("Failed to set DPI scaling: {result}");
            }

            info!("DPI scaling changed successfully");
        }

        Ok(())
    }
}
