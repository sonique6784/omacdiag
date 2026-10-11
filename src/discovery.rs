use crate::model::*;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

fn host_command(program: &str, args: &[&str], max: usize) -> Result<String, String> {
    let output = crate::process::capture(
        crate::process::command(program, args),
        None,
        Duration::from_secs(3),
        Arc::new(AtomicBool::new(false)),
        max,
    )?;
    if output.code != Some(0) {
        return Err(format!(
            "exit status {:?}: {}",
            output.code,
            output.stderr.trim()
        ));
    }
    if output.stdout.len() == max {
        return Err("output limit reached".into());
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

fn omarchy_version(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "Version")
            .then(|| value.trim().to_string())
            .filter(|s| !s.is_empty())
    })
}

fn pci_identifier(value: &str) -> Option<PciIdentifier> {
    let (name, id) = value.trim().strip_suffix(']')?.rsplit_once(" [")?;
    if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(PciIdentifier {
        name: name.to_string(),
        id: id.to_ascii_lowercase(),
    })
}

fn parse_pci_record(fields: &BTreeMap<&str, &str>) -> Option<PciDevice> {
    Some(PciDevice {
        address: fields.get("Slot")?.trim().to_string(),
        class: pci_identifier(fields.get("Class")?)?,
        vendor: pci_identifier(fields.get("Vendor")?)?,
        device: pci_identifier(fields.get("Device")?)?,
        subsystem_vendor: fields.get("SVendor").and_then(|v| pci_identifier(v)),
        subsystem_device: fields.get("SDevice").and_then(|v| pci_identifier(v)),
        revision: fields.get("Rev").map(|v| v.to_string()),
        programming_interface: fields.get("ProgIf").map(|v| v.to_string()),
        driver: fields.get("Driver").map(|v| v.to_string()),
    })
}

fn parse_lspci(output: &str) -> Result<(Vec<PciDevice>, usize), String> {
    let mut devices = Vec::new();
    let mut malformed = 0;
    let mut fields = BTreeMap::new();
    for line in output.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            if !fields.is_empty() {
                if let Some(device) = parse_pci_record(&fields) {
                    devices.push(device);
                } else {
                    malformed += 1;
                }
                fields.clear();
            }
        } else if let Some((key, value)) = line.split_once(':') {
            fields.insert(key.trim(), value.trim());
        } else {
            malformed += 1;
        }
    }
    if !output.trim().is_empty() && devices.is_empty() {
        return Err("no valid PCI records in lspci output".into());
    }
    devices.sort_by(|a, b| a.address.cmp(&b.address));
    Ok((devices, malformed))
}

fn collect_host_metadata(inventory: &mut Inventory) {
    inventory.machine.omarchy_version = host_command("pacman", &["-Qi", "omarchy"], 16 * 1024)
        .ok()
        .and_then(|output| omarchy_version(&output));
    match host_command("lspci", &["-D", "-vmm", "-nn", "-k"], 2 * 1024 * 1024)
        .and_then(|output| parse_lspci(&output))
    {
        Ok((devices, malformed)) => {
            inventory.pci_devices = Some(devices);
            if malformed > 0 {
                inventory.warnings.push(format!(
                    "lspci output contained {malformed} malformed record(s) or line(s)"
                ));
            }
        }
        Err(error) => inventory
            .warnings
            .push(format!("PCI inventory unavailable: {error}")),
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    #[test]
    fn parses_pacman_version_and_missing_package_information() {
        assert_eq!(
            omarchy_version("Name : omarchy\nVersion : 4.0.4-1\n"),
            Some("4.0.4-1".into())
        );
        assert_eq!(
            omarchy_version("error: package 'omarchy' was not found"),
            None
        );
        assert_eq!(omarchy_version("Version :  \n"), None);
    }

    #[test]
    fn parses_pci_names_ids_and_optional_fields() {
        let output = "Slot:\t0000:00:02.0\nClass:\tVGA compatible controller [0300]\nVendor:\tIntel Corporation [8086]\nDevice:\tIvy Bridge mobile GT2 [HD Graphics 4000] [0166]\nSVendor:\tApple Inc. [106b]\nSDevice:\tDevice [00fd]\nRev:\t09\nProgIf:\t00\nDriver:\ti915\n\nSlot:\t0000:02:00.0\nClass:\tNetwork controller [0280]\nVendor:\tBroadcom Inc. [14e4]\nDevice:\tBCM43224 [4353]\n\n";
        let (devices, malformed) = parse_lspci(output).unwrap();
        assert_eq!(malformed, 0);
        assert_eq!(devices.len(), 2);
        assert_eq!(
            devices[0].device.name,
            "Ivy Bridge mobile GT2 [HD Graphics 4000]"
        );
        assert_eq!(devices[0].device.id, "0166");
        assert_eq!(devices[0].driver.as_deref(), Some("i915"));
        assert_eq!(devices[0].subsystem_vendor.as_ref().unwrap().id, "106b");
        assert!(devices[1].driver.is_none());
        assert!(devices[1].subsystem_vendor.is_none());
    }

    #[test]
    fn rejects_unparseable_pci_output_without_inventing_devices() {
        assert!(parse_lspci("Slot:\t0000:00:02.0\nClass:\tbad\n").is_err());
        assert_eq!(parse_lspci("").unwrap(), (vec![], 0));
    }
}

#[derive(Clone)]
pub struct LinuxPaths {
    pub sys: PathBuf,
    pub proc: PathBuf,
    pub etc: PathBuf,
    pub dev: PathBuf,
}
impl Default for LinuxPaths {
    fn default() -> Self {
        Self {
            sys: "/sys".into(),
            proc: "/proc".into(),
            etc: "/etc".into(),
            dev: "/dev".into(),
        }
    }
}
pub fn read(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
pub fn entries(path: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    paths.sort();
    paths
}
fn name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into()
}
fn driver(path: &Path) -> Option<String> {
    [path.join("driver"), path.join("device/driver")]
        .iter()
        .find_map(|p| fs::read_link(p).ok().map(|p| name(&p)))
}
fn device(kind: Kind, path: &Path, title: String) -> Device {
    let mut d = Device::detected(kind, format!("{}:{}", kind.key(), name(path)), title);
    d.sysfs_path = Some(path.to_string_lossy().into());
    if let Ok(target) = fs::canonicalize(path) {
        d.attributes
            .insert("sysfs_target".into(), target.to_string_lossy().into());
    }
    d.driver = driver(path);
    d.evidence.push(format!("sysfs entry: {}", path.display()));
    d
}
fn attr(d: &mut Device, path: &Path, keys: &[&str]) {
    for key in keys {
        if let Some(value) = read(path.join(key)) {
            d.attributes.insert((*key).into(), value);
        }
    }
}
fn node(d: &mut Device, paths: &LinuxPaths, file: &str) {
    let path = paths.dev.join(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        if let Ok(metadata) = fs::metadata(&path) {
            let block = matches!(d.kind, Kind::Storage | Kind::Optical | Kind::SdReader);
            if (block && metadata.file_type().is_block_device())
                || (!block && metadata.file_type().is_char_device())
            {
                d.attributes
                    .insert("node_rdev".into(), metadata.rdev().to_string());
                d.node = Some(path.to_string_lossy().into());
                return;
            }
        }
    }
    d.evidence.push(format!(
        "No usable device node at {}; kernel enumeration alone does not establish access",
        path.display()
    ));
}

#[cfg(target_os = "linux")]
fn v4l2_device_caps(path: &Path) -> Option<u32> {
    use std::{mem::MaybeUninit, os::fd::AsRawFd};

    #[repr(C)]
    struct Capability {
        driver: [u8; 16],
        card: [u8; 32],
        bus_info: [u8; 32],
        version: u32,
        capabilities: u32,
        device_caps: u32,
        reserved: [u32; 3],
    }
    unsafe extern "C" {
        fn ioctl(fd: i32, request: usize, ...) -> i32;
    }
    // _IOR('V', 0, struct v4l2_capability), defined by linux/videodev2.h.
    const VIDIOC_QUERYCAP: usize = 0x8068_5600;
    let file = fs::File::open(path).ok()?;
    let mut caps = MaybeUninit::<Capability>::zeroed();
    if unsafe { ioctl(file.as_raw_fd(), VIDIOC_QUERYCAP, caps.as_mut_ptr()) } != 0 {
        return None;
    }
    let caps = unsafe { caps.assume_init() };
    Some(effective_v4l2_caps(caps.capabilities, caps.device_caps))
}

#[cfg(target_os = "linux")]
fn effective_v4l2_caps(global: u32, node: u32) -> u32 {
    const V4L2_CAP_DEVICE_CAPS: u32 = 0x8000_0000;
    if global & V4L2_CAP_DEVICE_CAPS != 0 {
        node
    } else {
        global
    }
}

#[cfg(target_os = "linux")]
fn v4l2_can_capture(caps: u32) -> bool {
    const V4L2_CAP_VIDEO_CAPTURE: u32 = 0x0000_0001;
    const V4L2_CAP_VIDEO_CAPTURE_MPLANE: u32 = 0x0000_1000;
    caps & (V4L2_CAP_VIDEO_CAPTURE | V4L2_CAP_VIDEO_CAPTURE_MPLANE) != 0
}

pub fn discover() -> Inventory {
    let mut inventory = discover_for_probe();
    if cfg!(target_os = "linux") {
        collect_host_metadata(&mut inventory);
    }
    inventory
}

pub fn discover_for_probe() -> Inventory {
    if cfg!(target_os = "linux") {
        discover_at(&LinuxPaths::default())
    } else {
        Inventory { schema_version: SCHEMA_VERSION,
        machine: Machine { manufacturer: "unknown".into(), model: "unknown".into(), serial: None,
            omarchy_version: None, board_name: None, board_version: None,
            os: std::env::consts::OS.into(), architecture: std::env::consts::ARCH.into(),
            distribution: "unsupported host".into(), kernel: "unknown".into(), model_profile: None,
            expected_ports: BTreeMap::new(), profile_sources: vec![] },
        pci_devices: None, devices: vec![], warnings: vec!["Hardware discovery and functional tests require Linux. This host can build the core and inspect fixture reports.".into()] }
    }
}

pub fn discover_at(p: &LinuxPaths) -> Inventory {
    let dmi = p.sys.join("class/dmi/id");
    let distribution = read(p.etc.join("os-release"))
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').into())
            })
        })
        .unwrap_or("unknown".into());
    let mut inventory = Inventory {
        schema_version: SCHEMA_VERSION,
        machine: Machine {
            manufacturer: read(dmi.join("sys_vendor")).unwrap_or("unknown".into()),
            model: read(dmi.join("product_name")).unwrap_or("unknown".into()),
            serial: read(dmi.join("product_serial")),
            omarchy_version: None,
            board_name: read(dmi.join("board_name")),
            board_version: read(dmi.join("board_version")),
            os: "linux".into(),
            distribution,
            kernel: read(p.proc.join("sys/kernel/osrelease")).unwrap_or("unknown".into()),
            architecture: std::env::consts::ARCH.into(),
            model_profile: None,
            expected_ports: BTreeMap::new(),
            profile_sources: vec![],
        },
        pci_devices: None,
        devices: vec![],
        warnings: vec![],
    };
    let devices = &mut inventory.devices;
    if let Some(cpu) = read(p.proc.join("cpuinfo")) {
        let model = cpu
            .lines()
            .find_map(|l| {
                l.strip_prefix("model name")
                    .and_then(|v| v.split_once(':').map(|(_, v)| v.trim()))
            })
            .unwrap_or("CPU");
        let mut d = Device::detected(Kind::Cpu, "cpu:system", model);
        d.attributes.insert(
            "logical_cpus".into(),
            cpu.lines()
                .filter(|l| l.starts_with("processor\t") || l.starts_with("processor "))
                .count()
                .to_string(),
        );
        devices.push(d);
    }
    if let Some(memory) = read(p.proc.join("meminfo")) {
        let mut d = Device::detected(Kind::Memory, "memory:system", "System memory");
        for key in ["MemTotal", "MemAvailable"] {
            if let Some(v) = memory
                .lines()
                .find_map(|l| l.strip_prefix(&format!("{key}:")).map(str::trim))
            {
                d.attributes.insert(key.into(), v.into());
            }
        }
        devices.push(d);
    }
    for path in entries(p.sys.join("bus/pci/devices")) {
        let class = read(path.join("class")).unwrap_or_default();
        let kind = if class.starts_with("0x03") {
            Some(Kind::Gpu)
        } else if class.starts_with("0x0c03") {
            Some(Kind::Usb)
        } else if class.starts_with("0x0c00") {
            Some(Kind::Firewire)
        } else if class.starts_with("0x0805") {
            Some(Kind::SdReader)
        } else if read(path.join("vendor")).as_deref() == Some("0x14e4")
            && read(path.join("device")).as_deref() == Some("0x1570")
        {
            Some(Kind::Camera)
        } else {
            None
        };
        if let Some(kind) = kind {
            let mut d = device(kind, &path, format!("{} PCI {}", kind.key(), name(&path)));
            attr(
                &mut d,
                &path,
                &[
                    "vendor",
                    "device",
                    "class",
                    "subsystem_vendor",
                    "subsystem_device",
                ],
            );
            if kind == Kind::Camera {
                d.name = "Broadcom PCIe FaceTime HD camera".into();
                d.evidence
                    .push("PCI enumeration does not establish V4L2 capture availability".into());
            }
            devices.push(d);
        }
    }
    for path in entries(p.sys.join("bus/usb/devices")) {
        if !path.join("idVendor").exists() {
            continue;
        }
        let mut d = device(
            Kind::Usb,
            &path,
            read(path.join("product")).unwrap_or(format!("USB {}", name(&path))),
        );
        attr(
            &mut d,
            &path,
            &[
                "idVendor",
                "idProduct",
                "serial",
                "manufacturer",
                "product",
                "speed",
                "busnum",
                "devpath",
                "maxchild",
            ],
        );
        d.attributes.insert(
            "role".into(),
            if name(&path).starts_with("usb") {
                "root_hub"
            } else {
                "device"
            }
            .into(),
        );
        let product = d.name.to_lowercase();
        if product.contains("card reader") || product.contains("sd reader") {
            let mut reader = device(Kind::SdReader, &path, d.name.clone());
            reader.attributes.insert("transport".into(), "usb".into());
            devices.push(reader);
        }
        devices.push(d);
    }
    for path in entries(p.sys.join("class/video4linux")) {
        let mut d = device(
            Kind::Camera,
            &path,
            read(path.join("name")).unwrap_or(name(&path)),
        );
        node(&mut d, p, &name(&path));
        #[cfg(target_os = "linux")]
        if let Some(node) = d.node.as_ref() {
            match v4l2_device_caps(Path::new(node)) {
                Some(caps) => {
                    d.attributes
                        .insert("v4l2_device_caps".into(), format!("0x{caps:08x}"));
                    if !v4l2_can_capture(caps) {
                        inventory.warnings.push(format!(
                            "{} is a V4L2 non-video endpoint and is not a separate camera",
                            node
                        ));
                        continue;
                    }
                }
                None => d.evidence.push(
                    "V4L2 capability query unavailable; capture role remains unconfirmed".into(),
                ),
            }
        }
        let underlying = fs::canonicalize(path.join("device")).ok();
        // Merge PCI inventory with the capture endpoint rather than testing one camera twice.
        if let Some(existing) = devices.iter_mut().find(|d| {
            d.kind == Kind::Camera
                && d.node.is_none()
                && d.sysfs_path.as_ref().is_some_and(|s| {
                    fs::canonicalize(s)
                        .ok()
                        .is_some_and(|pci| underlying.as_ref().is_some_and(|u| u.starts_with(pci)))
                })
        }) {
            existing.node = d.node;
            if let Some(rdev) = d.attributes.get("node_rdev") {
                existing.attributes.insert("node_rdev".into(), rdev.clone());
            }
            existing.evidence.extend(d.evidence);
        } else {
            devices.push(d);
        }
    }
    #[cfg(target_os = "linux")]
    for d in devices.iter_mut().filter(|d| d.kind == Kind::Camera) {
        let Some(node) = d.node.as_ref() else {
            continue;
        };
        let modes = crate::camera::query(Path::new(node));
        if let Some(maximum) = modes.maximum {
            d.attributes
                .insert("max_capture_width".into(), maximum.width.to_string());
            d.attributes
                .insert("max_capture_height".into(), maximum.height.to_string());
            d.attributes
                .insert("max_capture_format".into(), maximum.fourcc);
            if let Some(fps) = maximum.fps {
                d.attributes
                    .insert("max_capture_fps".into(), format!("{fps:.2}"));
            }
        } else {
            d.evidence
                .push("Maximum V4L2 capture resolution could not be enumerated".into());
        }
        if let Some(selected) = modes.selected {
            d.attributes
                .insert("selected_capture_width".into(), selected.width.to_string());
            d.attributes.insert(
                "selected_capture_height".into(),
                selected.height.to_string(),
            );
            d.attributes
                .insert("selected_capture_format".into(), selected.fourcc);
            if let Some(fps) = selected.fps {
                d.attributes
                    .insert("selected_capture_fps".into(), format!("{fps:.2}"));
            }
            if let Some(rate) = selected.frame_rate {
                d.attributes
                    .insert("selected_capture_frame_rate".into(), rate);
            }
        }
        if let Some(preview) = modes.preview {
            d.attributes
                .insert("preview_capture_width".into(), preview.width.to_string());
            d.attributes
                .insert("preview_capture_height".into(), preview.height.to_string());
            d.attributes
                .insert("preview_capture_format".into(), preview.fourcc);
            if let Some(fps) = preview.fps {
                d.attributes
                    .insert("preview_capture_fps".into(), format!("{fps:.2}"));
            }
            if let Some(rate) = preview.frame_rate {
                d.attributes
                    .insert("preview_capture_frame_rate".into(), rate);
            }
        }
    }
    for path in entries(p.sys.join("class/sound")) {
        let n = name(&path);
        if !n.starts_with("pcmC") {
            continue;
        }
        let Some((card, endpoint)) = n.strip_prefix("pcmC").and_then(|s| s.split_once('D')) else {
            continue;
        };
        let kind = if n.ends_with('c') {
            Kind::Microphone
        } else if n.ends_with('p') {
            Kind::Speaker
        } else {
            continue;
        };
        let card_id = read(p.sys.join(format!("class/sound/card{card}/id"))).unwrap_or(card.into());
        let pcm_number = endpoint.trim_end_matches(['c', 'p']);
        let direction = if kind == Kind::Microphone { 'c' } else { 'p' };
        let pcm_name = read(
            p.proc
                .join(format!("asound/card{card}/pcm{pcm_number}{direction}/info")),
        )
        .and_then(|info| {
            info.lines()
                .find_map(|line| line.strip_prefix("name:").map(str::trim))
                .map(str::to_string)
        });
        let mut d = device(
            kind,
            &path,
            format!(
                "{card_id} {}",
                pcm_name.as_deref().unwrap_or(if kind == Kind::Microphone {
                    "capture"
                } else {
                    "playback"
                })
            ),
        );
        if let Some(pcm_name) = pcm_name {
            let lower = pcm_name.to_lowercase();
            if kind == Kind::Speaker {
                d.attributes.insert(
                    "playback_route".into(),
                    if lower.contains("hdmi") || lower.contains("displayport") {
                        "display"
                    } else if lower.contains("analog") {
                        "analog"
                    } else {
                        "unknown"
                    }
                    .into(),
                );
            } else {
                d.attributes.insert(
                    "physical_input".into(),
                    (!lower.contains("loopback") && !lower.contains("monitor")).to_string(),
                );
            }
            d.attributes.insert("alsa_pcm_name".into(), pcm_name);
        }
        d.attributes.insert(
            "alsa_device".into(),
            format!("plughw:CARD={card_id},DEV={pcm_number}"),
        );
        d.attributes.insert("card".into(), card.into());
        if kind == Kind::Microphone {
            d.attributes
                .entry("physical_input".into())
                .or_insert_with(|| "true".into());
        }
        node(&mut d, p, &format!("snd/{n}"));
        devices.push(d);
    }
    for path in entries(p.sys.join("class/sound")) {
        let n = name(&path);
        if n.starts_with("card") && n[4..].chars().all(|c| c.is_ascii_digit()) {
            let mut d = device(
                Kind::AudioJack,
                &path,
                format!(
                    "{} audio routing",
                    read(path.join("id")).unwrap_or(n.clone())
                ),
            );
            d.attributes.insert("card".into(), n[4..].into());
            d.recognition = Recognition::Unknown;
            d.evidence.push("Sound card found; physical jack presence requires a model profile or operator verification".into());
            devices.push(d);
        }
    }
    for path in entries(p.sys.join("class/block")) {
        let n = name(&path);
        if path.join("partition").exists()
            || ["loop", "ram", "zram", "dm-", "md"]
                .iter()
                .any(|s| n.starts_with(s))
        {
            continue;
        }
        let kind = if n.starts_with("sr") {
            Kind::Optical
        } else {
            Kind::Storage
        };
        let mut d = device(
            kind,
            &path,
            read(path.join("device/model")).unwrap_or(n.clone()),
        );
        node(&mut d, p, &n);
        attr(
            &mut d,
            &path,
            &[
                "size",
                "removable",
                "ro",
                "device/model",
                "device/vendor",
                "device/serial",
            ],
        );
        devices.push(d);
        if n.starts_with("mmcblk") {
            let mut reader = device(Kind::SdReader, &path, "MMC/SD media endpoint".into());
            node(&mut reader, p, &n);
            devices.push(reader);
        }
    }
    for path in entries(p.sys.join("class/drm")) {
        let n = name(&path);
        if n.starts_with("renderD")
            && let Ok(parent) = fs::canonicalize(path.join("device"))
            && let Some(d) = devices.iter_mut().find(|d| {
                d.kind == Kind::Gpu
                    && d.sysfs_path
                        .as_ref()
                        .and_then(|s| fs::canonicalize(s).ok())
                        .as_ref()
                        == Some(&parent)
            })
        {
            node(d, p, &format!("dri/{n}"));
        }
        if path.join("status").exists() {
            let mut d = device(Kind::Display, &path, n.clone());
            attr(&mut d, &path, &["status", "enabled", "modes"]);
            d.attributes.insert(
                "internal".into(),
                (n.contains("eDP") || n.contains("LVDS") || n.contains("DSI")).to_string(),
            );
            if let Ok(edid) = fs::read(path.join("edid")) {
                d.attributes
                    .insert("edid_bytes".into(), edid.len().to_string());
            }
            devices.push(d);
        }
    }
    for path in entries(p.sys.join("class/power_supply")) {
        let kind = if read(path.join("type")).as_deref() == Some("Battery") {
            Kind::Battery
        } else {
            Kind::Power
        };
        let mut d = device(kind, &path, name(&path));
        attr(
            &mut d,
            &path,
            &[
                "type",
                "status",
                "online",
                "capacity",
                "energy_full",
                "energy_full_design",
                "charge_full",
                "charge_full_design",
                "cycle_count",
                "voltage_now",
                "current_now",
                "power_now",
            ],
        );
        devices.push(d);
    }
    for path in entries(p.sys.join("class/net")) {
        if name(&path) == "lo" {
            continue;
        }
        let mut d = device(Kind::Network, &path, name(&path));
        attr(&mut d, &path, &["operstate", "carrier", "speed", "address"]);
        d.attributes.insert(
            "wireless".into(),
            path.join("wireless").exists().to_string(),
        );
        devices.push(d);
    }
    for path in entries(p.sys.join("class/bluetooth")) {
        let mut d = device(Kind::Bluetooth, &path, name(&path));
        attr(&mut d, &path, &["address"]);
        devices.push(d);
    }
    for path in entries(p.sys.join("class/input")) {
        let n = name(&path);
        if !n.starts_with("event") {
            continue;
        }
        let title = read(path.join("device/name")).unwrap_or_default();
        let lower = title.to_lowercase();
        let keys = read(path.join("device/capabilities/key")).unwrap_or_default();
        let bit = |code: usize| {
            keys.split_whitespace()
                .rev()
                .nth(code / 64)
                .and_then(|s| u64::from_str_radix(s, 16).ok())
                .is_some_and(|v| v & (1_u64 << (code % 64)) != 0)
        };
        let kind = if lower.contains("touch bar") || lower.contains("touchbar") {
            Some(Kind::TouchBar)
        } else if bit(30) && bit(57) {
            Some(Kind::Keyboard)
        } else if bit(325)
            || lower.contains("bcm5974")
            || lower.contains("touchpad")
            || lower.contains("trackpad")
        {
            Some(Kind::Trackpad)
        } else if lower.contains("keyboard") {
            Some(Kind::Keyboard)
        } else {
            None
        };
        if let Some(kind) = kind {
            let mut d = device(kind, &path, title);
            node(&mut d, p, &format!("input/{n}"));
            devices.push(d);
        }
    }
    for path in entries(p.sys.join("class/hwmon")) {
        let label = read(path.join("name")).unwrap_or(name(&path));
        for input in entries(&path) {
            let n = name(&input);
            let kind = if n.starts_with("temp") && n.ends_with("_input") {
                Some(Kind::Thermal)
            } else if n.starts_with("fan") && n.ends_with("_input") {
                Some(Kind::Fan)
            } else {
                None
            };
            if let Some(kind) = kind {
                let mut d = device(kind, &path, format!("{label} {n}"));
                d.id.push_str(&format!(":{n}"));
                d.attributes
                    .insert("sensor_file".into(), input.to_string_lossy().into());
                devices.push(d);
            }
        }
    }
    for path in entries(p.sys.join("bus/thunderbolt/devices")) {
        let mut d = device(
            Kind::Thunderbolt,
            &path,
            read(path.join("device_name")).unwrap_or(name(&path)),
        );
        attr(&mut d, &path, &["authorized", "generation", "vendor_name"]);
        devices.push(d);
    }
    for path in entries(p.sys.join("class/leds")) {
        if name(&path).contains("kbd_backlight") {
            let mut d = device(Kind::KeyboardBacklight, &path, "Keyboard backlight".into());
            attr(&mut d, &path, &["brightness", "max_brightness"]);
            devices.push(d);
        }
    }
    crate::profiles::apply(&mut inventory);
    inventory.devices.sort_by(|a, b| a.id.cmp(&b.id));
    inventory
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn metadata_sibling_does_not_inherit_camera_capture_capability() {
        // A UVC camera can advertise both functions globally while each node has one role.
        let global = 0x84a0_0001;
        let video = effective_v4l2_caps(global, 0x0420_0001);
        let metadata = effective_v4l2_caps(global, 0x04a0_0000);
        assert!(v4l2_can_capture(video));
        assert!(!v4l2_can_capture(metadata));
        assert!(v4l2_can_capture(effective_v4l2_caps(0x0000_0001, 0)));
    }
}
