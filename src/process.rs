use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub static INTERRUPTED: AtomicBool = AtomicBool::new(false);
#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
    fn setpgid(pid: i32, pgid: i32) -> i32;
    fn signal(sig: i32, handler: usize) -> usize;
}
#[cfg(unix)]
extern "C" fn interrupted(_: i32) {
    INTERRUPTED.store(true, Ordering::Relaxed);
}
pub fn install_signals() {
    #[cfg(unix)]
    unsafe {
        signal(2, interrupted as *const () as usize);
        signal(15, interrupted as *const () as usize);
    }
}
pub fn isolated(command: &mut Command) {
    bind_parent(command);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
}

pub fn bind_parent(command: &mut Command) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        unsafe extern "C" {
            fn prctl(option: i32, ...) -> i32;
            fn getppid() -> i32;
        }
        let parent = std::process::id() as i32;
        // Prevent orphaned workers/capture programs after a hard controller crash.
        unsafe {
            command.pre_exec(move || {
                if prctl(1, 9_usize, 0_usize, 0_usize, 0_usize) != 0 || getppid() != parent {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = command;
}
pub fn terminate(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        kill(-(child.id() as i32), 9);
    }
    let _ = child.kill();
    let _ = child.wait();
}
pub struct Output {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

fn drain(mut stream: impl Read, max: usize) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut bytes = [0; 8192];
    loop {
        match stream.read(&mut bytes) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let remaining = max.saturating_sub(kept.len());
                kept.extend_from_slice(&bytes[..n.min(remaining)]);
            }
        }
    }
    kept
}

pub fn capture(
    mut command: Command,
    input: Option<Vec<u8>>,
    timeout: Duration,
    cancel: Arc<AtomicBool>,
    max: usize,
) -> Result<Output, String> {
    // Worker children inherit the worker group so controller cancellation kills the whole tree.
    if std::env::var_os("OMACDIAG_WORKER").is_none() {
        isolated(&mut command);
    } else {
        bind_parent(&mut command);
    }
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| format!("spawn: {e}"))?;
    let stdout = child.stdout.take().ok_or("Missing stdout")?;
    let stderr = child.stderr.take().ok_or("Missing stderr")?;
    let out = thread::spawn(move || drain(stdout, max));
    let err = thread::spawn(move || drain(stderr, 16384));
    let writer = input.map(|bytes| {
        let mut stdin = child.stdin.take().unwrap();
        thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        })
    });
    let start = Instant::now();
    let status;
    loop {
        if cancel.load(Ordering::Relaxed)
            || INTERRUPTED.load(Ordering::Relaxed)
            || start.elapsed() > timeout
        {
            let reason = if cancel.load(Ordering::Relaxed) || INTERRUPTED.load(Ordering::Relaxed) {
                "cancelled"
            } else {
                "deadline_exceeded"
            };
            terminate(&mut child);
            let _ = out.join();
            let _ = err.join();
            if let Some(w) = writer {
                let _ = w.join();
            }
            return Err(reason.into());
        }
        match child.try_wait() {
            Ok(Some(s)) => {
                status = s;
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(e) => {
                terminate(&mut child);
                return Err(e.to_string());
            }
        }
    }
    // Kill leftover subprocesses that inherited the pipes after their parent exited.
    #[cfg(unix)]
    unsafe {
        kill(-(child.id() as i32), 9);
    }
    if let Some(w) = writer {
        let _ = w.join();
    }
    Ok(Output {
        code: status.code(),
        stdout: out.join().unwrap_or_default(),
        stderr: String::from_utf8_lossy(&err.join().unwrap_or_default()).into(),
    })
}
pub fn command(program: &str, args: &[&str]) -> Command {
    let mut c = Command::new(program);
    c.args(args);
    c.env("LC_ALL", "C");
    c
}
