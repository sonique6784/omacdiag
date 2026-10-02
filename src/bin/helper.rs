// Policy-controlled helper. No arbitrary command, path, option, or write operation is accepted.
use std::{path::Path, process::Command};
fn main() {
    if let Err(e) = run() {
        eprintln!("omacdiag-helper: {e}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("Linux required".into());
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "smart" {
        return Err("Usage: omacdiag-helper smart BLOCK_BASENAME".into());
    }
    let name = &args[1];
    if name.is_empty() || name.len() > 64 || !name.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err("Invalid device basename".into());
    }
    let sys = Path::new("/sys/class/block").join(name);
    if !sys.exists() || sys.join("partition").exists() {
        return Err("Only whole kernel block devices are accepted".into());
    }
    let path = Path::new("/dev").join(name);
    let canonical = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
    if canonical.parent() != Some(Path::new("/dev")) || canonical.file_name() != path.file_name() {
        return Err("Device path must not redirect outside /dev".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let m = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if !m.file_type().is_block_device() {
            return Err("Not a block device".into());
        }
        let dev = m.rdev();
        let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & 0xfffff000);
        let minor = (dev & 0xff) | ((dev >> 12) & 0xffffff00);
        if omacdiag::discovery::read(sys.join("dev")).as_deref()
            != Some(&format!("{major}:{minor}"))
        {
            return Err("Kernel device identity mismatch".into());
        }
    }
    let tool = ["/usr/sbin/smartctl", "/usr/bin/smartctl"]
        .iter()
        .find(|p| Path::new(p).exists())
        .ok_or("smartctl is not installed")?;
    let mut command = Command::new(tool);
    command
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin")
        .env("LC_ALL", "C")
        .args(["--json", "--all"])
        .arg(path);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        Err(error.to_string())
    }
    #[cfg(not(unix))]
    {
        Err("Unix required".into())
    }
}
