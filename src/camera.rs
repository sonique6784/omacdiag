//! Read-only V4L2 mode enumeration. Capture remains in the media probe.
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub fourcc: String,
    pub fps: Option<f64>,
    pub frame_rate: Option<String>,
}

impl Mode {
    pub fn resolution(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    pub fn ffmpeg_format(&self) -> Option<&'static str> {
        match self.fourcc.as_str() {
            "MJPG" => Some("mjpeg"),
            "YUYV" => Some("yuyv422"),
            "UYVY" => Some("uyvy422"),
            "NV12" => Some("nv12"),
            "RGB3" => Some("rgb24"),
            "H264" => Some("h264"),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct Capabilities {
    pub maximum: Option<Mode>,
    pub selected: Option<Mode>,
    pub preview: Option<Mode>,
}

fn better(candidate: &Mode, previous: &Mode) -> bool {
    let candidate_area = u64::from(candidate.width) * u64::from(candidate.height);
    let previous_area = u64::from(previous.width) * u64::from(previous.height);
    candidate_area > previous_area
        || (candidate_area == previous_area
            && (candidate.fps.unwrap_or(0.0) > previous.fps.unwrap_or(0.0)
                || (candidate.fps == previous.fps && candidate.width > previous.width)))
}

fn select(modes: impl IntoIterator<Item = Mode>) -> Capabilities {
    let mut capabilities = Capabilities::default();
    for mode in modes {
        if capabilities
            .maximum
            .as_ref()
            .is_none_or(|previous| better(&mode, previous))
        {
            capabilities.maximum = Some(mode.clone());
        }
        if mode.ffmpeg_format().is_some()
            && capabilities
                .selected
                .as_ref()
                .is_none_or(|previous| better(&mode, previous))
        {
            capabilities.selected = Some(mode.clone());
        }
        if mode.ffmpeg_format().is_some() && mode.width >= 320 && mode.height >= 180 {
            let preview_fps = mode.fps.unwrap_or(0.0).min(30.0);
            let preferred = capabilities.preview.as_ref().is_none_or(|previous| {
                let previous_fps = previous.fps.unwrap_or(0.0).min(30.0);
                preview_fps > previous_fps + 1.0
                    || ((preview_fps - previous_fps).abs() <= 1.0
                        && u64::from(mode.width) * u64::from(mode.height)
                            < u64::from(previous.width) * u64::from(previous.height))
            });
            if preferred {
                capabilities.preview = Some(mode);
            }
        }
    }
    capabilities
}

#[cfg(target_os = "linux")]
pub fn query(path: &Path) -> Capabilities {
    use std::{fs::File, mem::size_of, os::fd::AsRawFd};

    #[repr(C)]
    #[derive(Default)]
    struct Format {
        index: u32,
        kind: u32,
        flags: u32,
        description: [u8; 32],
        pixel_format: u32,
        mbus_code: u32,
        reserved: [u32; 3],
    }
    #[repr(C)]
    #[derive(Default)]
    struct FrameSize {
        index: u32,
        pixel_format: u32,
        kind: u32,
        data: [u32; 6],
        reserved: [u32; 2],
    }
    #[repr(C)]
    #[derive(Default)]
    struct FrameInterval {
        index: u32,
        pixel_format: u32,
        width: u32,
        height: u32,
        kind: u32,
        data: [u32; 6],
        reserved: [u32; 2],
    }
    unsafe extern "C" {
        fn ioctl(fd: i32, request: usize, ...) -> i32;
    }
    fn iowr(number: usize, size: usize) -> usize {
        0xc000_0000 | (size << 16) | (b'V' as usize) << 8 | number
    }
    let Ok(file) = File::open(path) else {
        return Capabilities::default();
    };
    let fd = file.as_raw_fd();
    let mut modes = Vec::new();
    for format_index in 0..256 {
        let mut format = Format {
            index: format_index,
            kind: 1, // V4L2_BUF_TYPE_VIDEO_CAPTURE
            ..Default::default()
        };
        if unsafe { ioctl(fd, iowr(2, size_of::<Format>()), &mut format) } != 0 {
            break;
        }
        let fourcc = format.pixel_format.to_le_bytes();
        let fourcc = String::from_utf8_lossy(&fourcc).into_owned();
        for size_index in 0..512 {
            let mut size = FrameSize {
                index: size_index,
                pixel_format: format.pixel_format,
                ..Default::default()
            };
            if unsafe { ioctl(fd, iowr(74, size_of::<FrameSize>()), &mut size) } != 0 {
                break;
            }
            let (width, height) = match size.kind {
                1 => (size.data[0], size.data[1]),     // discrete
                2 | 3 => (size.data[1], size.data[4]), // continuous or stepwise maxima
                _ => continue,
            };
            if width == 0 || height == 0 {
                continue;
            }
            let mut fastest: Option<(f64, String)> = None;
            for interval_index in 0..512 {
                let mut interval = FrameInterval {
                    index: interval_index,
                    pixel_format: format.pixel_format,
                    width,
                    height,
                    ..Default::default()
                };
                if unsafe { ioctl(fd, iowr(75, size_of::<FrameInterval>()), &mut interval) } != 0 {
                    break;
                }
                let (numerator, denominator) = (interval.data[0], interval.data[1]);
                if numerator == 0 || denominator == 0 {
                    continue;
                }
                let fps = f64::from(denominator) / f64::from(numerator);
                if fastest.as_ref().is_none_or(|(current, _)| fps > *current) {
                    fastest = Some((fps, format!("{denominator}/{numerator}")));
                }
                if interval.kind != 1 {
                    break; // stepwise/continuous: first fraction is the minimum interval
                }
            }
            modes.push(Mode {
                width,
                height,
                fourcc: fourcc.clone(),
                fps: fastest.as_ref().map(|(fps, _)| *fps),
                frame_rate: fastest.map(|(_, rate)| rate),
            });
            if size.kind != 1 {
                break;
            }
        }
    }
    select(modes)
}

#[cfg(not(target_os = "linux"))]
pub fn query(_path: &Path) -> Capabilities {
    Capabilities::default()
}

pub fn selected_from_device(device: &crate::model::Device) -> Option<Mode> {
    mode_from_device(device, "selected")
}

pub fn preview_from_device(device: &crate::model::Device) -> Option<Mode> {
    mode_from_device(device, "preview")
}

fn mode_from_device(device: &crate::model::Device, prefix: &str) -> Option<Mode> {
    let attrs = &device.attributes;
    Some(Mode {
        width: attrs
            .get(&format!("{prefix}_capture_width"))?
            .parse()
            .ok()?,
        height: attrs
            .get(&format!("{prefix}_capture_height"))?
            .parse()
            .ok()?,
        fourcc: attrs.get(&format!("{prefix}_capture_format"))?.clone(),
        fps: attrs
            .get(&format!("{prefix}_capture_fps"))
            .and_then(|s| s.parse().ok()),
        frame_rate: attrs.get(&format!("{prefix}_capture_frame_rate")).cloned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_fastest_mode_at_largest_resolution() {
        let mode = |width, height, fourcc: &str, fps| Mode {
            width,
            height,
            fourcc: fourcc.into(),
            fps: Some(fps),
            frame_rate: None,
        };
        let modes = select([
            mode(320, 240, "YUYV", 29.97),
            mode(640, 480, "YUYV", 30.0),
            mode(1280, 720, "YUYV", 10.0),
            mode(1280, 720, "MJPG", 29.97),
        ]);
        assert_eq!(modes.maximum.as_ref().unwrap().resolution(), "1280x720");
        assert_eq!(modes.selected.unwrap().fourcc, "MJPG");
        assert_eq!(modes.preview.unwrap().resolution(), "320x240");
    }

    #[test]
    fn unknown_format_is_advertised_but_not_selected() {
        let modes = select([Mode {
            width: 1920,
            height: 1080,
            fourcc: "ABCD".into(),
            fps: None,
            frame_rate: None,
        }]);
        assert_eq!(modes.maximum.unwrap().resolution(), "1920x1080");
        assert!(modes.selected.is_none());
    }
}
