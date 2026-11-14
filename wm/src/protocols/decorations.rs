//! Server-side decorations: apps are told not to draw their own title bars
//! (`xdg-decoration`, server side always), and wm draws a border round each
//! window instead, `BORDER` px wide in the gap around it, in the colour of
//! the first seat focusing it, else the inactive colour. A full-screen
//! window has none. The colours are the config file's `colors`.
//!
//! Windows have rounded corners (`RADIUS`): their own surfaces are drawn
//! with a texture shader that cuts them to the window's geometry
//! (`Rounded`), and the border is a ring round them drawn by a pixel shader.
//! Popups (menus) aren't cut; full-screen windows are square.
//!
//! GTK 4 apps don't speak `xdg-decoration` and keep their own; kitty,
//! Firefox, Qt and others drop theirs.

use std::sync::Mutex;

use serde_json::Value;
use smithay::{
    backend::renderer::{
        buffer_y_inverted,
        element::{
            AsRenderElements, Element, Id, Kind, RenderElement, UnderlyingStorage,
            surface::{WaylandSurfaceRenderElement, render_elements_from_surface_tree},
        },
        gles::{
            GlesError, GlesFrame, GlesPixelProgram, GlesRenderer, GlesTexProgram, Uniform, UniformName, UniformType,
            UniformValue, element::PixelShaderElement,
        },
        utils::{CommitCounter, DamageSet, OpaqueRegions},
    },
    desktop::{PopupManager, Window},
    reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode,
    utils::{Buffer as BufferCoords, Logical, Physical, Point, Rectangle, Scale, Transform, user_data::UserDataMap},
    wayland::shell::xdg::{
        ToplevelSurface,
        decoration::XdgDecorationHandler,
    },
};

use crate::state::State;

/// The border's width, in the gap around the window.
pub const BORDER: i32 = 2;

/// The colours seats and borders are drawn in.
#[derive(Debug, Clone, PartialEq)]
pub struct Colors {
    /// Seat N's pointer and focus border: the Nth, round again past the end.
    pub seats: Vec<[f32; 4]>,
    /// The border of a window no seat focuses.
    pub inactive: [f32; 4],
}

impl Colors {
    pub fn seat(&self, index: usize) -> [f32; 4] {
        self.seats.get(index % self.seats.len().max(1)).copied().unwrap_or([1.0; 4])
    }

    /// `{"seats": ["#rrggbb", …], "inactive": "#rrggbb"}`, either key over
    /// `self`.
    pub fn merged(&self, value: &Value) -> Result<Self, String> {
        let Value::Object(map) = value else { return Err("colors must be an object".into()) };
        let mut colors = self.clone();
        for (key, value) in map {
            match (key.as_str(), value) {
                ("seats", Value::Array(list)) if !list.is_empty() => {
                    colors.seats = list.iter().map(color).collect::<Result<_, _>>()?;
                }
                ("inactive", value) => colors.inactive = color(value)?,
                _ => return Err(format!("colors: unknown or malformed key {key:?}")),
            }
        }
        Ok(colors)
    }
}

/// `"#rrggbb"` or `"#rrggbbaa"`.
fn color(value: &Value) -> Result<[f32; 4], String> {
    let text = value.as_str().ok_or("a colour is a \"#rrggbb\" string")?;
    let hex = text.strip_prefix('#').filter(|h| matches!(h.len(), 6 | 8) && h.is_ascii()).ok_or(format!("bad colour {text:?}"))?;
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map(|b| b as f32 / 255.0).map_err(|_| format!("bad colour {text:?}"));
    Ok([byte(0)?, byte(2)?, byte(4)?, if hex.len() == 8 { byte(6)? } else { 1.0 }])
}

/// A window's border element, kept while its place and colour hold, so
/// damage tracking sees it unchanged.
#[derive(Default)]
struct Border(Mutex<Option<(BorderKey, PixelShaderElement)>>);

/// What a border is drawn for: its area, colour and alpha (as bits).
type BorderKey = (Rectangle<i32, Logical>, [u32; 4], u32);

impl State {
    /// `window`'s border, its geometry at `loc` on screen: a ring `BORDER`
    /// px wide round its rounded corners. None without shaders.
    pub fn border_of(&self, renderer: &mut GlesRenderer, window: &Window, loc: Point<i32, Logical>, alpha: f32) -> Option<PixelShaderElement> {
        let shaders = shaders(renderer)?;
        let seat = self.focusing(window).next();
        let color = seat.map_or(self.config.colors.inactive, |s| self.config.colors.seat(s));
        let size = window.geometry().size;
        let area = Rectangle::new(loc - Point::from((BORDER, BORDER)), (size.w + 2 * BORDER, size.h + 2 * BORDER).into());
        let key = (area, color.map(f32::to_bits), alpha.to_bits());
        let border = window.user_data().get_or_insert_threadsafe(Border::default);
        let mut cached = border.0.lock().unwrap();
        if let Some((old, element)) = cached.as_ref()
            && *old == key
        {
            return Some(element.clone());
        }
        let element = PixelShaderElement::new(
            shaders.border,
            area,
            None,
            alpha,
            vec![
                Uniform::new("color", color),
                Uniform::new("radius", (RADIUS + BORDER) as f32),
                Uniform::new("width", BORDER as f32),
            ],
            Kind::Unspecified,
        );
        *cached = Some((key, element.clone()));
        Some(element)
    }
}

/// The corners' radius, inside the border.
pub const RADIUS: i32 = 8;

/// A rounded rectangle's signed distance: `p` from its centre, `extent`
/// its half size (`half` is reserved in GLSL ES).
const ROUNDED_BOX: &str = "
float rounded_box(vec2 p, vec2 extent, float r) {
    vec2 q = abs(p) - extent + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}
";

/// A ring: the rounded rectangle filling the element less one `width`
/// smaller with `radius - width` corners.
const BORDER_SHADER: &str = "
precision highp float;
uniform float alpha;
uniform vec2 size;
varying vec2 v_coords;
uniform vec4 color;
uniform float radius;
uniform float width;
//ROUNDED_BOX
void main() {
    vec2 p = v_coords * size - size / 2.0;
    float outer = clamp(0.5 - rounded_box(p, size / 2.0, radius), 0.0, 1.0);
    float inner = clamp(0.5 - rounded_box(p, size / 2.0 - width, max(radius - width, 0.0)), 0.0, 1.0);
    float a = outer * (1.0 - inner) * color.a * alpha;
    gl_FragColor = vec4(color.rgb * a, a);
}
";

/// Smithay's texture shader, cut to the window's geometry with rounded
/// corners: `input_to_geo` takes texture coordinates to the geometry's
/// (0 to 1 across it).
const CLIP_SHADER: &str = "#version 100
//_DEFINES_
#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif
precision highp float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif
uniform float alpha;
varying vec2 v_coords;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif
uniform vec2 geo_size;
uniform float radius;
uniform mat3 input_to_geo;
//ROUNDED_BOX
void main() {
    vec4 color = texture2D(tex, v_coords);
#if defined(NO_ALPHA)
    color = vec4(color.rgb, 1.0);
#endif
    vec2 geo = (input_to_geo * vec3(v_coords, 1.0)).xy;
    if (geo.x < 0.0 || geo.x > 1.0 || geo.y < 0.0 || geo.y > 1.0) {
        color = vec4(0.0);
    } else {
        vec2 p = geo * geo_size - geo_size / 2.0;
        color *= clamp(0.5 - rounded_box(p, geo_size / 2.0, radius), 0.0, 1.0);
    }
    color *= alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}
";

/// The shaders, compiled once per renderer.
#[derive(Clone)]
struct Shaders {
    border: GlesPixelProgram,
    clip: GlesTexProgram,
}

/// `renderer`'s shaders; None (logged once) if they don't compile.
fn shaders(renderer: &mut GlesRenderer) -> Option<Shaders> {
    if let Some(shaders) = renderer.egl_context().user_data().get::<Option<Shaders>>() {
        return shaders.clone();
    }
    let border = renderer.compile_custom_pixel_shader(
        BORDER_SHADER.replace("//ROUNDED_BOX", ROUNDED_BOX),
        &[
            UniformName::new("color", UniformType::_4f),
            UniformName::new("radius", UniformType::_1f),
            UniformName::new("width", UniformType::_1f),
        ],
    );
    let clip = renderer.compile_custom_texture_shader(
        CLIP_SHADER.replace("//ROUNDED_BOX", ROUNDED_BOX),
        &[
            UniformName::new("geo_size", UniformType::_2f),
            UniformName::new("radius", UniformType::_1f),
            UniformName::new("input_to_geo", UniformType::Matrix3x3),
        ],
    );
    let shaders = match (border, clip) {
        (Ok(border), Ok(clip)) => Some(Shaders { border, clip }),
        (Err(err), _) | (_, Err(err)) => {
            tracing::warn!(?err, "border shaders: square corners, no borders");
            None
        }
    };
    renderer.egl_context().user_data().insert_if_missing(|| shaders.clone());
    shaders
}

/// A window's surface, drawn only inside its geometry, corners rounded.
#[derive(Debug)]
pub struct Rounded {
    inner: WaylandSurfaceRenderElement<GlesRenderer>,
    program: GlesTexProgram,
    /// The window's geometry on screen.
    geo: Rectangle<i32, Logical>,
}

impl Rounded {
    fn uniforms(&self) -> Vec<Uniform<'static>> {
        let elem = self.inner.geometry(Scale::from(1.0));
        let buf = self.inner.buffer_size();
        let src = self.inner.view().src;
        let inverted = buffer_y_inverted(self.inner.buffer()).unwrap_or(false);
        // Per axis, `input_to_geo` is `x * s + t`: texture to buffer
        // (flipped if need be), to the surface's source rectangle, to the
        // element, to the geometry.
        let axis = |inverted: bool, buf: f64, src: (f64, f64), elem: (i32, i32), geo: (i32, i32)| {
            let (s, t) = if inverted { (-1.0, 1.0) } else { (1.0, 0.0) };
            let (k, c) = (buf / src.1, -src.0 / src.1);
            let (s, t) = (s * k, t * k + c);
            let (k, c) = (elem.1 as f64 / geo.1 as f64, (elem.0 - geo.0) as f64 / geo.1 as f64);
            ((s * k) as f32, (t * k + c) as f32)
        };
        let (sx, tx) = axis(false, buf.w as f64, (src.loc.x, src.size.w), (elem.loc.x, elem.size.w), (self.geo.loc.x, self.geo.size.w));
        let (sy, ty) = axis(inverted, buf.h as f64, (src.loc.y, src.size.h), (elem.loc.y, elem.size.h), (self.geo.loc.y, self.geo.size.h));
        // Column major.
        let matrix = [sx, 0.0, 0.0, 0.0, sy, 0.0, tx, ty, 1.0];
        vec![
            Uniform::new("geo_size", (self.geo.size.w as f32, self.geo.size.h as f32)),
            Uniform::new("radius", RADIUS as f32),
            Uniform::new("input_to_geo", UniformValue::Matrix3x3 { matrices: vec![matrix], transpose: false }),
        ]
    }

    /// The geometry's corners, which aren't opaque.
    fn corners(&self) -> [Rectangle<i32, Logical>; 4] {
        let Rectangle { loc, size } = self.geo;
        let r = RADIUS;
        [(loc.x, loc.y), (loc.x + size.w - r, loc.y), (loc.x, loc.y + size.h - r), (loc.x + size.w - r, loc.y + size.h - r)]
            .map(|at| Rectangle::new(at.into(), (r, r).into()))
    }
}

impl Element for Rounded {
    fn id(&self) -> &Id {
        self.inner.id()
    }
    fn current_commit(&self) -> CommitCounter {
        self.inner.current_commit()
    }
    fn src(&self) -> Rectangle<f64, BufferCoords> {
        self.inner.src()
    }
    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.inner.geometry(scale)
    }
    fn transform(&self) -> Transform {
        self.inner.transform()
    }
    fn damage_since(&self, scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        self.inner.damage_since(scale, commit)
    }
    /// The surface's, inside the geometry, less the corners.
    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        let origin = self.inner.geometry(scale).loc;
        let local = |r: Rectangle<i32, Logical>| {
            let mut r = r.to_physical(1);
            r.loc -= origin;
            r
        };
        let geo = local(self.geo);
        let regions = self.inner.opaque_regions(scale).into_iter().filter_map(|r| r.intersection(geo));
        OpaqueRegions::from_slice(&Rectangle::subtract_rects_many(regions, self.corners().map(local)))
    }
    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }
    fn kind(&self) -> Kind {
        self.inner.kind()
    }
}

impl RenderElement<GlesRenderer> for Rounded {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, BufferCoords>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        frame.override_default_tex_program(self.program.clone(), self.uniforms());
        let result = RenderElement::<GlesRenderer>::draw(&self.inner, frame, src, dst, damage, opaque_regions, cache);
        frame.clear_tex_program_override();
        result
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        None
    }
}

smithay::backend::renderer::element::render_elements! {
    pub WindowElement<=GlesRenderer>;
    Rounded=Rounded,
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Border=PixelShaderElement,
}

/// `window`'s surfaces with its geometry at `geo` on screen: its popups,
/// then its own surfaces, cut to the geometry with rounded corners unless
/// `square` (full screen) or there are no shaders.
pub fn window_surfaces(
    renderer: &mut GlesRenderer,
    window: &Window,
    geo: Rectangle<i32, Logical>,
    alpha: f32,
    square: bool,
) -> (Vec<WindowElement>, Vec<WindowElement>) {
    let scale = Scale::from(1.0);
    let at = (geo.loc - window.geometry().loc).to_physical(1);
    let Some(toplevel) = window.toplevel() else {
        let all = window.render_elements::<WaylandSurfaceRenderElement<_>>(renderer, at, scale, alpha);
        return (Vec::new(), all.into_iter().map(WindowElement::Surface).collect());
    };
    let surface = toplevel.wl_surface();
    let popups = PopupManager::popups_for_surface(surface)
        .flat_map(|(popup, offset)| {
            let offset = (window.geometry().loc + offset - popup.geometry().loc).to_physical(1);
            render_elements_from_surface_tree(renderer, popup.wl_surface(), at + offset, scale, alpha, Kind::Unspecified)
        })
        .map(WindowElement::Surface)
        .collect();
    let program = if square { None } else { shaders(renderer).map(|s| s.clip) };
    let own = render_elements_from_surface_tree::<_, WaylandSurfaceRenderElement<_>>(renderer, surface, at, scale, alpha, Kind::Unspecified)
        .into_iter()
        .map(|inner| match (&program, inner.transform()) {
            (Some(program), Transform::Normal) => WindowElement::Rounded(Rounded { inner, program: program.clone(), geo }),
            _ => WindowElement::Surface(inner),
        })
        .collect();
    (popups, own)
}

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        server_side(&toplevel);
    }

    /// Whatever it prefers, wm draws them.
    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: Mode) {
        server_side(&toplevel);
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        server_side(&toplevel);
    }

    fn decoration_destroyed(&mut self, _toplevel: ToplevelSurface) {}
}

fn server_side(toplevel: &ToplevelSurface) {
    toplevel.with_pending_state(|s| s.decoration_mode = Some(Mode::ServerSide));
    if toplevel.is_initial_configure_sent() {
        toplevel.send_pending_configure();
    }
}
