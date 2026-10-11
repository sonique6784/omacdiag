use super::*;

const WIDTH: i32 = 640;
const HEIGHT: i32 = 480;
const GL_COLOR_BUFFER_BIT: u32 = 0x4000;
const GL_DEPTH_BUFFER_BIT: u32 = 0x0100;
const GL_TEXTURE_2D: u32 = 0x0de1;
const GL_RGBA: u32 = 0x1908;
const GL_UNSIGNED_BYTE: u32 = 0x1401;
const GL_FRAMEBUFFER: u32 = 0x8d40;
const GL_RENDERBUFFER: u32 = 0x8d41;
const GL_ARRAY_BUFFER: u32 = 0x8892;

pub struct ThreeDResult {
    pub vendor: String,
    pub renderer: String,
    pub gl_version: String,
    pub glsl_version: String,
    pub max_texture_size: i32,
    pub max_renderbuffer_size: i32,
    pub max_vertex_attribs: i32,
    pub max_texture_image_units: i32,
    pub frames: u64,
    pub seconds: f64,
}

struct Gl {
    get_string: unsafe extern "C" fn(u32) -> *const c_char,
    get_integer: unsafe extern "C" fn(u32, *mut i32),
    get_error: unsafe extern "C" fn() -> u32,
    create_shader: unsafe extern "C" fn(u32) -> u32,
    shader_source: unsafe extern "C" fn(u32, i32, *const *const c_char, *const i32),
    compile_shader: unsafe extern "C" fn(u32),
    get_shader: unsafe extern "C" fn(u32, u32, *mut i32),
    shader_log: unsafe extern "C" fn(u32, i32, *mut i32, *mut c_char),
    create_program: unsafe extern "C" fn() -> u32,
    attach_shader: unsafe extern "C" fn(u32, u32),
    link_program: unsafe extern "C" fn(u32),
    get_program: unsafe extern "C" fn(u32, u32, *mut i32),
    program_log: unsafe extern "C" fn(u32, i32, *mut i32, *mut c_char),
    use_program: unsafe extern "C" fn(u32),
    attrib_location: unsafe extern "C" fn(u32, *const c_char) -> i32,
    uniform_location: unsafe extern "C" fn(u32, *const c_char) -> i32,
    uniform_float: unsafe extern "C" fn(i32, f32),
    uniform_int: unsafe extern "C" fn(i32, i32),
    gen_textures: unsafe extern "C" fn(i32, *mut u32),
    bind_texture: unsafe extern "C" fn(u32, u32),
    tex_parameter: unsafe extern "C" fn(u32, u32, i32),
    tex_image: unsafe extern "C" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void),
    gen_framebuffers: unsafe extern "C" fn(i32, *mut u32),
    bind_framebuffer: unsafe extern "C" fn(u32, u32),
    framebuffer_texture: unsafe extern "C" fn(u32, u32, u32, u32, i32),
    framebuffer_status: unsafe extern "C" fn(u32) -> u32,
    gen_renderbuffers: unsafe extern "C" fn(i32, *mut u32),
    bind_renderbuffer: unsafe extern "C" fn(u32, u32),
    renderbuffer_storage: unsafe extern "C" fn(u32, u32, i32, i32),
    framebuffer_renderbuffer: unsafe extern "C" fn(u32, u32, u32, u32),
    gen_buffers: unsafe extern "C" fn(i32, *mut u32),
    bind_buffer: unsafe extern "C" fn(u32, u32),
    buffer_data: unsafe extern "C" fn(u32, isize, *const c_void, u32),
    enable_attrib: unsafe extern "C" fn(u32),
    attrib_pointer: unsafe extern "C" fn(u32, i32, u32, u8, i32, *const c_void),
    viewport: unsafe extern "C" fn(i32, i32, i32, i32),
    enable: unsafe extern "C" fn(u32),
    depth_func: unsafe extern "C" fn(u32),
    clear_color: unsafe extern "C" fn(f32, f32, f32, f32),
    clear_depth: unsafe extern "C" fn(f32),
    clear: unsafe extern "C" fn(u32),
    draw_arrays: unsafe extern "C" fn(u32, i32, i32),
    read_pixels: unsafe extern "C" fn(i32, i32, i32, i32, u32, u32, *mut c_void),
    finish: unsafe extern "C" fn(),
}

impl Gl {
    unsafe fn load(session: &GpuSession) -> Result<Self, Failure> {
        macro_rules! symbol {
            ($name:literal) => {
                unsafe {
                    session
                        .symbol(CStr::from_bytes_with_nul(concat!($name, "\0").as_bytes()).unwrap())
                }?
            };
        }
        Ok(Self {
            get_string: symbol!("glGetString"),
            get_integer: symbol!("glGetIntegerv"),
            get_error: symbol!("glGetError"),
            create_shader: symbol!("glCreateShader"),
            shader_source: symbol!("glShaderSource"),
            compile_shader: symbol!("glCompileShader"),
            get_shader: symbol!("glGetShaderiv"),
            shader_log: symbol!("glGetShaderInfoLog"),
            create_program: symbol!("glCreateProgram"),
            attach_shader: symbol!("glAttachShader"),
            link_program: symbol!("glLinkProgram"),
            get_program: symbol!("glGetProgramiv"),
            program_log: symbol!("glGetProgramInfoLog"),
            use_program: symbol!("glUseProgram"),
            attrib_location: symbol!("glGetAttribLocation"),
            uniform_location: symbol!("glGetUniformLocation"),
            uniform_float: symbol!("glUniform1f"),
            uniform_int: symbol!("glUniform1i"),
            gen_textures: symbol!("glGenTextures"),
            bind_texture: symbol!("glBindTexture"),
            tex_parameter: symbol!("glTexParameteri"),
            tex_image: symbol!("glTexImage2D"),
            gen_framebuffers: symbol!("glGenFramebuffers"),
            bind_framebuffer: symbol!("glBindFramebuffer"),
            framebuffer_texture: symbol!("glFramebufferTexture2D"),
            framebuffer_status: symbol!("glCheckFramebufferStatus"),
            gen_renderbuffers: symbol!("glGenRenderbuffers"),
            bind_renderbuffer: symbol!("glBindRenderbuffer"),
            renderbuffer_storage: symbol!("glRenderbufferStorage"),
            framebuffer_renderbuffer: symbol!("glFramebufferRenderbuffer"),
            gen_buffers: symbol!("glGenBuffers"),
            bind_buffer: symbol!("glBindBuffer"),
            buffer_data: symbol!("glBufferData"),
            enable_attrib: symbol!("glEnableVertexAttribArray"),
            attrib_pointer: symbol!("glVertexAttribPointer"),
            viewport: symbol!("glViewport"),
            enable: symbol!("glEnable"),
            depth_func: symbol!("glDepthFunc"),
            clear_color: symbol!("glClearColor"),
            clear_depth: symbol!("glClearDepthf"),
            clear: symbol!("glClear"),
            draw_arrays: symbol!("glDrawArrays"),
            read_pixels: symbol!("glReadPixels"),
            finish: symbol!("glFinish"),
        })
    }

    unsafe fn string(&self, name: u32) -> Result<String, Failure> {
        let p = unsafe { (self.get_string)(name) };
        if p.is_null() {
            return Err((
                "gpu_setup_failed",
                format!("GL string 0x{name:x} unavailable"),
            ));
        }
        Ok(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }

    unsafe fn integer(&self, name: u32) -> i32 {
        let mut value = 0;
        unsafe { (self.get_integer)(name, &mut value) };
        value
    }

    unsafe fn check(&self, stage: &str, reason: &'static str) -> Result<(), Failure> {
        let error = unsafe { (self.get_error)() };
        if error == 0 {
            Ok(())
        } else {
            Err((reason, format!("OpenGL error 0x{error:x} during {stage}")))
        }
    }

    unsafe fn shader(&self, kind: u32, source: &CStr) -> Result<u32, Failure> {
        let shader = unsafe { (self.create_shader)(kind) };
        if shader == 0 {
            return Err(("shader_failed", "Shader allocation failed".into()));
        }
        let source_ptr = source.as_ptr();
        unsafe {
            (self.shader_source)(shader, 1, &source_ptr, ptr::null());
            (self.compile_shader)(shader);
        }
        let mut compiled = 0;
        unsafe { (self.get_shader)(shader, 0x8b81, &mut compiled) };
        if compiled == 0 {
            let mut log = [0_u8; 2048];
            let mut length = 0;
            unsafe {
                (self.shader_log)(
                    shader,
                    log.len() as i32,
                    &mut length,
                    log.as_mut_ptr().cast(),
                )
            };
            return Err((
                "shader_failed",
                format!(
                    "Shader compilation failed: {}",
                    String::from_utf8_lossy(&log[..length.clamp(0, log.len() as i32) as usize])
                ),
            ));
        }
        Ok(shader)
    }
}

const VERTEX_SHADER: &CStr = c"attribute vec3 a_position;\nattribute vec2 a_uv;\nattribute vec3 a_tint;\nuniform float u_angle;\nvarying vec2 v_uv;\nvarying vec3 v_tint;\nvoid main() {\n  float c = cos(u_angle);\n  float s = sin(u_angle);\n  vec3 p = vec3(c*a_position.x+s*a_position.z, a_position.y, -s*a_position.x+c*a_position.z);\n  gl_Position = vec4(p, 1.0 - 0.35*p.z);\n  v_uv = a_uv;\n  v_tint = a_tint;\n}";
const FRAGMENT_SHADER: &CStr = c"precision mediump float;\nuniform sampler2D u_texture;\nvarying vec2 v_uv;\nvarying vec3 v_tint;\nvoid main() {\n  gl_FragColor = texture2D(u_texture, v_uv) * vec4(v_tint, 1.0);\n}";

const VERTICES: [[f32; 8]; 9] = [
    [-1.0, -1.0, 0.5, 0.0, 0.0, 1.0, 1.0, 1.0],
    [1.0, -1.0, 0.5, 1.0, 0.0, 1.0, 1.0, 1.0],
    [-1.0, 1.0, 0.5, 0.0, 1.0, 1.0, 1.0, 1.0],
    [-1.0, 1.0, 0.5, 0.0, 1.0, 1.0, 1.0, 1.0],
    [1.0, -1.0, 0.5, 1.0, 0.0, 1.0, 1.0, 1.0],
    [1.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0],
    [-0.35, -0.35, -0.5, 0.75, 0.75, 1.0, 1.0, 0.0],
    [0.35, -0.35, -0.5, 0.75, 0.75, 1.0, 1.0, 0.0],
    [0.0, 0.4, -0.5, 0.75, 0.75, 1.0, 1.0, 0.0],
];

fn pixel_matches(actual: [u8; 4], expected: [u8; 4]) -> bool {
    actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 8)
}

pub fn render_3d(device: &Device, seconds: u64) -> Result<ThreeDResult, Failure> {
    let session = open(device)?;
    // Context and library stay alive for every GL call in this scope.
    unsafe {
        let gl = Gl::load(&session)?;
        let vendor = gl.string(0x1f00)?;
        let gl_version = gl.string(0x1f02)?;
        let glsl_version = gl.string(0x8b8c)?;
        let max_texture_size = gl.integer(0x0d33);
        let max_renderbuffer_size = gl.integer(0x84e8);
        let max_vertex_attribs = gl.integer(0x8869);
        let max_texture_image_units = gl.integer(0x8872);
        gl.check("capability query", "gpu_setup_failed")?;
        if max_texture_size < WIDTH || max_renderbuffer_size < WIDTH {
            return Err((
                "gpu_setup_failed",
                "GPU limits are below the fixed 640×480 workload".into(),
            ));
        }

        let vertex = gl.shader(0x8b31, VERTEX_SHADER)?;
        let fragment = gl.shader(0x8b30, FRAGMENT_SHADER)?;
        let program = (gl.create_program)();
        if program == 0 {
            return Err(("shader_failed", "Program allocation failed".into()));
        }
        (gl.attach_shader)(program, vertex);
        (gl.attach_shader)(program, fragment);
        (gl.link_program)(program);
        let mut linked = 0;
        (gl.get_program)(program, 0x8b82, &mut linked);
        if linked == 0 {
            let mut log = [0_u8; 2048];
            let mut length = 0;
            (gl.program_log)(
                program,
                log.len() as i32,
                &mut length,
                log.as_mut_ptr().cast(),
            );
            return Err((
                "shader_failed",
                format!(
                    "Program link failed: {}",
                    String::from_utf8_lossy(&log[..length.clamp(0, log.len() as i32) as usize])
                ),
            ));
        }
        (gl.use_program)(program);
        let position = (gl.attrib_location)(program, c"a_position".as_ptr());
        let uv = (gl.attrib_location)(program, c"a_uv".as_ptr());
        let tint = (gl.attrib_location)(program, c"a_tint".as_ptr());
        let angle = (gl.uniform_location)(program, c"u_angle".as_ptr());
        let sampler = (gl.uniform_location)(program, c"u_texture".as_ptr());
        if [position, uv, tint, angle, sampler].iter().any(|&v| v < 0) {
            return Err((
                "shader_failed",
                "Required shader input was optimized out or unavailable".into(),
            ));
        }
        (gl.uniform_int)(sampler, 0);

        let mut scene_texture = 0;
        (gl.gen_textures)(1, &mut scene_texture);
        (gl.bind_texture)(GL_TEXTURE_2D, scene_texture);
        (gl.tex_parameter)(GL_TEXTURE_2D, 0x2801, 0x2600); // min: nearest
        (gl.tex_parameter)(GL_TEXTURE_2D, 0x2800, 0x2600); // mag: nearest
        (gl.tex_parameter)(GL_TEXTURE_2D, 0x2802, 0x812f); // wrap S: clamp to edge
        (gl.tex_parameter)(GL_TEXTURE_2D, 0x2803, 0x812f); // wrap T: clamp to edge
        let texels: [u8; 16] = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        (gl.tex_image)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA as i32,
            2,
            2,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            texels.as_ptr().cast(),
        );

        let mut color = 0;
        (gl.gen_textures)(1, &mut color);
        (gl.bind_texture)(GL_TEXTURE_2D, color);
        (gl.tex_parameter)(GL_TEXTURE_2D, 0x2801, 0x2600);
        (gl.tex_parameter)(GL_TEXTURE_2D, 0x2800, 0x2600);
        (gl.tex_image)(
            GL_TEXTURE_2D,
            0,
            GL_RGBA as i32,
            WIDTH,
            HEIGHT,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            ptr::null(),
        );
        let mut depth = 0;
        (gl.gen_renderbuffers)(1, &mut depth);
        (gl.bind_renderbuffer)(GL_RENDERBUFFER, depth);
        (gl.renderbuffer_storage)(GL_RENDERBUFFER, 0x81a5, WIDTH, HEIGHT); // depth16
        let mut framebuffer = 0;
        (gl.gen_framebuffers)(1, &mut framebuffer);
        (gl.bind_framebuffer)(GL_FRAMEBUFFER, framebuffer);
        (gl.framebuffer_texture)(GL_FRAMEBUFFER, 0x8ce0, GL_TEXTURE_2D, color, 0);
        (gl.framebuffer_renderbuffer)(GL_FRAMEBUFFER, 0x8d00, GL_RENDERBUFFER, depth);
        if (gl.framebuffer_status)(GL_FRAMEBUFFER) != 0x8cd5 {
            return Err((
                "framebuffer_incomplete",
                "Offscreen color/depth framebuffer is incomplete".into(),
            ));
        }
        (gl.bind_texture)(GL_TEXTURE_2D, scene_texture);

        let mut buffer = 0;
        (gl.gen_buffers)(1, &mut buffer);
        (gl.bind_buffer)(GL_ARRAY_BUFFER, buffer);
        (gl.buffer_data)(
            GL_ARRAY_BUFFER,
            std::mem::size_of_val(&VERTICES) as isize,
            VERTICES.as_ptr().cast(),
            0x88e4,
        ); // static draw
        for (location, size, offset) in [(position, 3, 0), (uv, 2, 12), (tint, 3, 20)] {
            (gl.enable_attrib)(location as u32);
            (gl.attrib_pointer)(
                location as u32,
                size,
                0x1406,
                0,
                32,
                offset as *const c_void,
            );
        }
        (gl.viewport)(0, 0, WIDTH, HEIGHT);
        (gl.enable)(0x0b71); // depth test
        (gl.depth_func)(0x0201); // less
        (gl.clear_color)(0.0, 0.0, 0.0, 1.0);
        (gl.clear_depth)(1.0);
        gl.check("3D setup", "gpu_setup_failed")?;

        let draw = |near_angle: f32| {
            (gl.clear)(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
            (gl.uniform_float)(angle, near_angle);
            (gl.draw_arrays)(0x0004, 6, 3); // near textured triangle
            (gl.uniform_float)(angle, 0.0);
            (gl.draw_arrays)(0x0004, 0, 6); // far textured quad must remain behind the triangle
            (gl.finish)();
        };
        draw(0.0);
        for (x, y, expected) in [
            (80, 80, [255, 0, 0, 255]),
            (560, 80, [0, 255, 0, 255]),
            (80, 400, [0, 0, 255, 255]),
            (560, 400, [255, 255, 255, 255]),
            (320, 240, [255, 255, 0, 255]),
        ] {
            let mut actual = [0_u8; 4];
            (gl.read_pixels)(
                x,
                y,
                1,
                1,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                actual.as_mut_ptr().cast(),
            );
            gl.check("3D readback", "gpu_operation_failed")?;
            if !pixel_matches(actual, expected) {
                return Err((
                    "readback_mismatch",
                    format!(
                        "3D pixel mismatch at ({x},{y}): expected {expected:?}, observed {actual:?}"
                    ),
                ));
            }
        }
        let warmup = Instant::now();
        let mut frame = 0_u64;
        while warmup.elapsed() < Duration::from_millis(500) {
            draw((frame % 360) as f32 * std::f32::consts::PI / 180.0);
            frame += 1;
        }
        gl.check("3D warmup", "gpu_setup_failed")?;
        let start = Instant::now();
        let mut frames = 0_u64;
        let mut last_thermal_check = start;
        while start.elapsed() < Duration::from_secs(seconds) {
            draw((frames % 360) as f32 * std::f32::consts::PI / 180.0);
            gl.check("3D rendering", "gpu_operation_failed")?;
            frames += 1;
            if last_thermal_check.elapsed() >= Duration::from_secs(1) {
                if crate::probes::basic::thermal_limit() {
                    return Err(("thermal_limit", "Stopped at 90°C sensor threshold".into()));
                }
                last_thermal_check = Instant::now();
            }
        }
        let elapsed = start.elapsed().as_secs_f64();
        Ok(ThreeDResult {
            vendor,
            renderer: session.renderer.clone(),
            gl_version,
            glsl_version,
            max_texture_size,
            max_renderbuffer_size,
            max_vertex_attribs,
            max_texture_image_units,
            frames,
            seconds: elapsed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::pixel_matches;

    #[test]
    fn wrong_3d_pixel_is_not_accepted() {
        assert!(pixel_matches([252, 255, 2, 255], [255, 255, 0, 255]));
        assert!(!pixel_matches([0, 255, 0, 255], [255, 255, 0, 255]));
    }
}
