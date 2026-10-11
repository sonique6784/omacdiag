use super::*;

pub fn execute(req: &WorkerRequest, d: &Device) -> TestResult {
    #[cfg(target_os = "linux")]
    {
        if req.test.probe == "gpu_3d" {
            return match linux::render_3d(
                d,
                if req.profile == Profile::Extended {
                    15
                } else {
                    3
                },
            ) {
                Ok(result) => {
                    let mut r = TestResult::new(
                        &req.test,
                        Outcome::Passed,
                        "gpu_3d_verified",
                        "Selected GPU rendered textured 3D geometry with correct depth and pixel readback",
                    );
                    r.measure("vendor", result.vendor, "identifier");
                    r.measure("renderer", result.renderer, "identifier");
                    r.measure("gl_version", result.gl_version, "version");
                    r.measure("glsl_version", result.glsl_version, "version");
                    r.measure("max_texture_size", result.max_texture_size, "pixels");
                    r.measure(
                        "max_renderbuffer_size",
                        result.max_renderbuffer_size,
                        "pixels",
                    );
                    r.measure("max_vertex_attribs", result.max_vertex_attribs, "count");
                    r.measure(
                        "max_texture_image_units",
                        result.max_texture_image_units,
                        "count",
                    );
                    r.measure("offscreen_frames", result.frames, "frames");
                    r.measure("offscreen_seconds", result.seconds, "s");
                    r.measure(
                        "offscreen_fps",
                        result.frames as f64 / result.seconds,
                        "fps",
                    );
                    r.measure("offscreen_resolution", "640×480", "pixels");
                    r.measure("render_node", &d.node, "path");
                    r.evidence.push("Synchronized offscreen throughput on a fixed workload; FPS is not displayed refresh rate or a pass threshold. This does not certify all 3D features or VRAM.".into());
                    r
                }
                Err((reason, detail)) => {
                    TestResult::new(&req.test, gpu_3d_outcome(reason), reason, detail)
                }
            };
        }
        match linux::render(
            d,
            if req.profile == Profile::Extended {
                60
            } else {
                2
            },
        ) {
            Ok((renderer, iterations)) => {
                let mut r = TestResult::new(
                    &req.test,
                    Outcome::Passed,
                    "hardware_readback_verified",
                    "Selected GPU rendered colour quadrants with matching pixel readback",
                );
                r.measure("renderer", renderer, "identifier");
                r.measure("iterations", iterations, "count");
                r.measure("render_node", &d.node, "path");
                r.evidence.push("32×32 EGL pbuffer checks basic rendering/readback; it is not a full VRAM or 3D workload test".into());
                r
            }
            Err((reason, detail)) => TestResult::new(
                &req.test,
                if reason == "readback_mismatch" {
                    Outcome::Failed
                } else if reason == "thermal_limit" {
                    Outcome::Inconclusive
                } else {
                    Outcome::Unsupported
                },
                reason,
                detail,
            ),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = d;
        TestResult::new(
            &req.test,
            Outcome::Unsupported,
            "linux_required",
            "Linux EGL backend required",
        )
    }
}

#[cfg(target_os = "linux")]
fn gpu_3d_outcome(reason: &str) -> Outcome {
    match reason {
        "readback_mismatch" | "gpu_operation_failed" => Outcome::Failed,
        "thermal_limit" | "gpu_setup_failed" | "shader_failed" | "framebuffer_incomplete" => {
            Outcome::Inconclusive
        }
        _ => Outcome::Unsupported,
    }
}

#[cfg(all(test, target_os = "linux"))]
mod outcome_tests {
    use super::*;

    #[test]
    fn rendering_errors_and_environment_limits_have_distinct_outcomes() {
        assert_eq!(gpu_3d_outcome("readback_mismatch"), Outcome::Failed);
        assert_eq!(gpu_3d_outcome("gpu_operation_failed"), Outcome::Failed);
        assert_eq!(gpu_3d_outcome("shader_failed"), Outcome::Inconclusive);
        assert_eq!(gpu_3d_outcome("thermal_limit"), Outcome::Inconclusive);
        assert_eq!(gpu_3d_outcome("render_node_missing"), Outcome::Unsupported);
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use crate::model::Device;
    use std::{
        ffi::{CStr, c_char, c_void},
        ptr,
        time::{Duration, Instant},
    };
    type Handle = *mut c_void;
    type Failure = (&'static str, String);
    mod three_d;
    pub use three_d::render_3d;
    #[link(name = "dl")]
    unsafe extern "C" {
        fn dlopen(name: *const c_char, flags: i32) -> Handle;
        fn dlsym(handle: Handle, name: *const c_char) -> Handle;
        fn dlclose(handle: Handle) -> i32;
    }
    struct Library(Handle);
    impl Drop for Library {
        fn drop(&mut self) {
            unsafe {
                dlclose(self.0);
            }
        }
    }
    impl Library {
        unsafe fn symbol<T: Copy>(&self, name: &CStr) -> Result<T, Failure> {
            let p = unsafe { dlsym(self.0, name.as_ptr()) };
            if p.is_null() {
                return Err(("egl_symbol_missing", name.to_string_lossy().into()));
            }
            // All callers supply an ABI-correct function pointer type with pointer-sized storage.
            Ok(unsafe { std::mem::transmute_copy(&p) })
        }
    }
    struct Context {
        display: Handle,
        surface: Handle,
        context: Handle,
        destroy_surface: unsafe extern "C" fn(Handle, Handle) -> u32,
        destroy_context: unsafe extern "C" fn(Handle, Handle) -> u32,
        terminate: unsafe extern "C" fn(Handle) -> u32,
        make_current: unsafe extern "C" fn(Handle, Handle, Handle, Handle) -> u32,
    }
    impl Drop for Context {
        fn drop(&mut self) {
            unsafe {
                (self.make_current)(
                    self.display,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
                if !self.context.is_null() {
                    (self.destroy_context)(self.display, self.context);
                }
                if !self.surface.is_null() {
                    (self.destroy_surface)(self.display, self.surface);
                }
                (self.terminate)(self.display);
            }
        }
    }
    unsafe fn extension<T: Copy>(
        get: unsafe extern "C" fn(*const c_char) -> Handle,
        name: &CStr,
    ) -> Result<T, Failure> {
        let p = unsafe { get(name.as_ptr()) };
        if p.is_null() {
            return Err(("egl_extension_missing", name.to_string_lossy().into()));
        }
        Ok(unsafe { std::mem::transmute_copy(&p) })
    }
    struct GpuSession {
        // The EGL context must be destroyed before its shared library is unloaded.
        _context: Context,
        _library: Library,
        get: unsafe extern "C" fn(*const c_char) -> Handle,
        renderer: String,
    }
    impl GpuSession {
        unsafe fn symbol<T: Copy>(&self, name: &CStr) -> Result<T, Failure> {
            unsafe { extension(self.get, name) }
        }
    }
    fn open(device: &Device) -> Result<GpuSession, Failure> {
        let node = device.node.as_ref().ok_or((
            "render_node_missing",
            "No render node is exposed for this GPU".into(),
        ))?;
        // Every symbol is checked; extensions are never invoked with null pointers.
        unsafe {
            let handle = dlopen(c"libEGL.so.1".as_ptr(), 2);
            if handle.is_null() {
                return Err(("egl_unavailable", "libEGL.so.1 is unavailable".into()));
            }
            let library = Library(handle);
            let get: unsafe extern "C" fn(*const c_char) -> Handle =
                library.symbol(c"eglGetProcAddress")?;
            let query: unsafe extern "C" fn(i32, *mut Handle, *mut i32) -> u32 =
                extension(get, c"eglQueryDevicesEXT")?;
            let string: unsafe extern "C" fn(Handle, i32) -> *const c_char =
                extension(get, c"eglQueryDeviceStringEXT")?;
            let platform: unsafe extern "C" fn(u32, Handle, *const i32) -> Handle =
                extension(get, c"eglGetPlatformDisplayEXT")?;
            let mut count = 0;
            if query(0, ptr::null_mut(), &mut count) == 0 || count <= 0 || count > 256 {
                return Err((
                    "egl_devices_unavailable",
                    "Cannot enumerate EGL devices".into(),
                ));
            }
            let mut devices = vec![ptr::null_mut(); count as usize];
            if query(count, devices.as_mut_ptr(), &mut count) == 0 {
                return Err(("egl_devices_unavailable", "EGL enumeration failed".into()));
            }
            let selected = devices
                .into_iter()
                .take(count as usize)
                .find(|d| {
                    let p = string(*d, 0x3377);
                    !p.is_null() && CStr::from_ptr(p).to_string_lossy() == node.as_str()
                })
                .ok_or((
                    "gpu_mapping_unavailable",
                    "EGL device cannot be mapped to the selected DRM render node; no default-GPU substitution was made".into(),
                ))?;
            let display = platform(0x313f, selected, ptr::null());
            if display.is_null() {
                return Err((
                    "egl_display_unavailable",
                    "Selected GPU display unavailable".into(),
                ));
            }
            let initialize: unsafe extern "C" fn(Handle, *mut i32, *mut i32) -> u32 =
                library.symbol(c"eglInitialize")?;
            let choose: unsafe extern "C" fn(
                Handle,
                *const i32,
                *mut Handle,
                i32,
                *mut i32,
            ) -> u32 = library.symbol(c"eglChooseConfig")?;
            let bind: unsafe extern "C" fn(u32) -> u32 = library.symbol(c"eglBindAPI")?;
            let pbuffer: unsafe extern "C" fn(Handle, Handle, *const i32) -> Handle =
                library.symbol(c"eglCreatePbufferSurface")?;
            let create: unsafe extern "C" fn(Handle, Handle, Handle, *const i32) -> Handle =
                library.symbol(c"eglCreateContext")?;
            let destroy_surface = library.symbol(c"eglDestroySurface")?;
            let destroy_context = library.symbol(c"eglDestroyContext")?;
            let terminate = library.symbol(c"eglTerminate")?;
            let make_current = library.symbol(c"eglMakeCurrent")?;
            if initialize(display, ptr::null_mut(), ptr::null_mut()) == 0 {
                return Err((
                    "egl_initialize_failed",
                    "Driver could not initialize this GPU".into(),
                ));
            }
            let mut context = Context {
                display,
                surface: ptr::null_mut(),
                context: ptr::null_mut(),
                destroy_surface,
                destroy_context,
                terminate,
                make_current,
            };
            let attrs = [
                0x3033, 1, 0x3040, 4, 0x3024, 8, 0x3023, 8, 0x3022, 8, 0x3021, 8, 0x3038,
            ];
            let mut config = ptr::null_mut();
            let mut configs = 0;
            if choose(display, attrs.as_ptr(), &mut config, 1, &mut configs) == 0
                || configs == 0
                || bind(0x30a0) == 0
            {
                return Err((
                    "egl_config_unavailable",
                    "No GLES2 pbuffer configuration is available".into(),
                ));
            }
            context.surface = pbuffer(display, config, [0x3057, 32, 0x3056, 32, 0x3038].as_ptr());
            context.context = create(
                display,
                config,
                ptr::null_mut(),
                [0x3098, 2, 0x3038].as_ptr(),
            );
            if context.surface.is_null()
                || context.context.is_null()
                || (context.make_current)(
                    display,
                    context.surface,
                    context.surface,
                    context.context,
                ) == 0
            {
                return Err((
                    "egl_context_unavailable",
                    "No usable context on the selected GPU".into(),
                ));
            }
            let gl_string: unsafe extern "C" fn(u32) -> *const c_char =
                extension(get, c"glGetString")?;
            let p = gl_string(0x1f01);
            if p.is_null() {
                return Err((
                    "renderer_unknown",
                    "GL renderer identification unavailable".into(),
                ));
            }
            let renderer = CStr::from_ptr(p).to_string_lossy().into_owned();
            let lower = renderer.to_lowercase();
            if [
                "llvmpipe",
                "softpipe",
                "swrast",
                "swiftshader",
                "software",
                "lavapipe",
            ]
            .iter()
            .any(|s| lower.contains(s))
            {
                return Err((
                    "software_renderer",
                    format!("Software renderer cannot verify a hardware GPU: {renderer}"),
                ));
            }
            Ok(GpuSession {
                _context: context,
                _library: library,
                get,
                renderer,
            })
        }
    }
    pub fn render(device: &Device, seconds: u64) -> Result<(String, u64), Failure> {
        let session = open(device)?;
        unsafe {
            let get = session.get;
            let renderer = session.renderer.as_str();
            let enable: unsafe extern "C" fn(u32) = extension(get, c"glEnable")?;
            let scissor: unsafe extern "C" fn(i32, i32, i32, i32) = extension(get, c"glScissor")?;
            let clear_color: unsafe extern "C" fn(f32, f32, f32, f32) =
                extension(get, c"glClearColor")?;
            let clear: unsafe extern "C" fn(u32) = extension(get, c"glClear")?;
            let pixels: unsafe extern "C" fn(i32, i32, i32, i32, u32, u32, Handle) =
                extension(get, c"glReadPixels")?;
            let error: unsafe extern "C" fn() -> u32 = extension(get, c"glGetError")?;
            let colours = [
                [255_u8, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [255, 255, 255, 255],
            ];
            let mut buffer = vec![0_u8; 32 * 32 * 4];
            let start = Instant::now();
            let mut iterations: u64 = 0;
            enable(0x0c11);
            while start.elapsed() < Duration::from_secs(seconds) {
                for (q, c) in colours.iter().enumerate() {
                    scissor((q % 2) as i32 * 16, (q / 2) as i32 * 16, 16, 16);
                    clear_color(
                        c[0] as f32 / 255.0,
                        c[1] as f32 / 255.0,
                        c[2] as f32 / 255.0,
                        1.0,
                    );
                    clear(0x00004000);
                }
                pixels(0, 0, 32, 32, 0x1908, 0x1401, buffer.as_mut_ptr().cast());
                if error() != 0 {
                    return Err((
                        "gpu_operation_failed",
                        "OpenGL error during rendering/readback".into(),
                    ));
                }
                for y in 0..32 {
                    for x in 0..32 {
                        let expected = colours[(y / 16) * 2 + x / 16];
                        let offset = (y * 32 + x) * 4;
                        if buffer[offset..offset + 4]
                            .iter()
                            .zip(expected)
                            .any(|(a, b)| a.abs_diff(b) > 2)
                        {
                            return Err((
                                "readback_mismatch",
                                format!("Readback mismatch at ({x},{y}) on {renderer}"),
                            ));
                        }
                    }
                }
                iterations += 1;
                if iterations.is_multiple_of(100) && crate::probes::basic::thermal_limit() {
                    return Err(("thermal_limit", "Stopped at 90°C sensor threshold".into()));
                }
            }
            Ok((renderer.to_string(), iterations))
        }
    }
}
