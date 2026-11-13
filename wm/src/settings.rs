//! Input settings, the config file's `input`: the keyboard's layout and
//! repeat (every seat's), touchpads' and mice's libinput options (on real
//! hardware, as each device connects and again on `reload`), and focus
//! following the mouse.
//!
//! ```json
//! "input": {
//!   "keyboard": {"layout": "us", "variant": "", "options": "", "repeat_rate": 25, "repeat_delay": 600},
//!   "touchpad": {"tap": true, "natural_scroll": true, "drag": true, "click_method": "clickfinger", "dwt": true},
//!   "mouse": {"natural_scroll": false, "accel_speed": 0.0},
//!   "focus_follows_mouse": true
//! }
//! ```
//!
//! A pointer option left out keeps libinput's default.

use serde_json::{Map, Value};
use smithay::{input::keyboard::XkbConfig, reexports::input::{self as libinput, DeviceCapability}};

#[derive(Debug, Clone, PartialEq)]
pub struct InputConfig {
    pub keyboard: Keyboard,
    pub touchpad: Pointer,
    pub mouse: Pointer,
    /// A window gets the seat's keyboard when its pointer moves onto it.
    pub focus_follows_mouse: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keyboard {
    pub layout: String,
    pub variant: String,
    pub options: String,
    pub model: String,
    /// Repeats a second.
    pub repeat_rate: i32,
    /// Milliseconds before repeating.
    pub repeat_delay: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pointer {
    pub tap: Option<bool>,
    pub drag: Option<bool>,
    pub natural_scroll: Option<bool>,
    /// Disable while typing.
    pub dwt: Option<bool>,
    pub click_method: Option<ClickMethod>,
    /// -1 to 1.
    pub accel_speed: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickMethod {
    Clickfinger,
    ButtonAreas,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            keyboard: Keyboard {
                layout: "us".into(),
                variant: String::new(),
                options: String::new(),
                model: String::new(),
                repeat_rate: 25,
                repeat_delay: 600,
            },
            touchpad: Pointer::default(),
            mouse: Pointer::default(),
            focus_follows_mouse: false,
        }
    }
}

impl InputConfig {
    /// `value`'s keys over `self`'s.
    pub fn merged(&self, value: &Value) -> Result<Self, String> {
        let map = object(value, "input")?;
        let mut new = self.clone();
        for (key, value) in map {
            match key.as_str() {
                "keyboard" => new.keyboard = new.keyboard.merged(value)?,
                "touchpad" => new.touchpad = new.touchpad.merged(value, "touchpad")?,
                "mouse" => new.mouse = new.mouse.merged(value, "mouse")?,
                "focus_follows_mouse" => new.focus_follows_mouse = boolean(value, key)?,
                _ => return Err(format!("input: unknown key {key:?}")),
            }
        }
        Ok(new)
    }

    /// Set a libinput device's options: a touchpad's or a mouse's.
    pub fn apply(&self, device: &mut libinput::Device) {
        let options = if device.config_tap_finger_count() > 0 {
            &self.touchpad
        } else if device.has_capability(DeviceCapability::Pointer) {
            &self.mouse
        } else {
            return;
        };
        let name = String::from(device.name());
        let mut results = Vec::new();
        if let Some(on) = options.tap {
            results.push(("tap", device.config_tap_set_enabled(on)));
        }
        if let Some(on) = options.drag {
            results.push(("drag", device.config_tap_set_drag_enabled(on)));
        }
        if let Some(on) = options.natural_scroll
            && device.config_scroll_has_natural_scroll()
        {
            results.push(("natural_scroll", device.config_scroll_set_natural_scroll_enabled(on)));
        }
        if let Some(on) = options.dwt
            && device.config_dwt_is_available()
        {
            results.push(("dwt", device.config_dwt_set_enabled(on)));
        }
        if let Some(method) = options.click_method {
            let method = match method {
                ClickMethod::Clickfinger => libinput::ClickMethod::Clickfinger,
                ClickMethod::ButtonAreas => libinput::ClickMethod::ButtonAreas,
            };
            if device.config_click_methods().contains(&method) {
                results.push(("click_method", device.config_click_set_method(method)));
            }
        }
        if let Some(speed) = options.accel_speed
            && device.config_accel_is_available()
        {
            results.push(("accel_speed", device.config_accel_set_speed(speed)));
        }
        for (option, result) in results {
            if let Err(err) = result {
                tracing::warn!(device = %name, "input option {option} not set: {err:?}");
            }
        }
    }
}

impl Keyboard {
    fn merged(&self, value: &Value) -> Result<Self, String> {
        let mut new = self.clone();
        for (key, value) in object(value, "keyboard")? {
            let text = || value.as_str().map(String::from).ok_or(format!("keyboard {key} must be a string"));
            let number = || {
                value.as_i64().filter(|&n| (1..=10_000).contains(&n)).map(|n| n as i32).ok_or(format!("keyboard {key} must be a positive number"))
            };
            match key.as_str() {
                "layout" => new.layout = text()?,
                "variant" => new.variant = text()?,
                "options" => new.options = text()?,
                "model" => new.model = text()?,
                "repeat_rate" => new.repeat_rate = number()?,
                "repeat_delay" => new.repeat_delay = number()?,
                _ => return Err(format!("keyboard: unknown key {key:?}")),
            }
        }
        Ok(new)
    }

    pub fn xkb(&self) -> XkbConfig<'_> {
        XkbConfig {
            layout: &self.layout,
            variant: &self.variant,
            model: &self.model,
            options: (!self.options.is_empty()).then(|| self.options.clone()),
            ..Default::default()
        }
    }
}

impl Pointer {
    fn merged(&self, value: &Value, name: &str) -> Result<Self, String> {
        let mut new = self.clone();
        for (key, value) in object(value, name)? {
            match key.as_str() {
                "tap" => new.tap = Some(boolean(value, key)?),
                "drag" => new.drag = Some(boolean(value, key)?),
                "natural_scroll" => new.natural_scroll = Some(boolean(value, key)?),
                "dwt" => new.dwt = Some(boolean(value, key)?),
                "click_method" => {
                    new.click_method = Some(match value.as_str() {
                        Some("clickfinger") => ClickMethod::Clickfinger,
                        Some("button_areas") => ClickMethod::ButtonAreas,
                        _ => return Err(format!("{name} click_method must be \"clickfinger\" or \"button_areas\"")),
                    })
                }
                "accel_speed" => {
                    new.accel_speed =
                        Some(value.as_f64().filter(|s| (-1.0..=1.0).contains(s)).ok_or(format!("{name} accel_speed must be from -1 to 1"))?)
                }
                _ => return Err(format!("{name}: unknown key {key:?}")),
            }
        }
        Ok(new)
    }
}

fn object<'a>(value: &'a Value, name: &str) -> Result<&'a Map<String, Value>, String> {
    value.as_object().ok_or(format!("{name} must be an object"))
}

fn boolean(value: &Value, key: &str) -> Result<bool, String> {
    value.as_bool().ok_or(format!("{key} must be true or false"))
}
