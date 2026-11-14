//! wm: a scrolling Wayland compositor with several seats and animations per
//! workspace. `main.rs` only starts it; everything lives here so the tests in
//! `tests/` can reach it. The command-line options and the config file are
//! below.

pub mod activation;
pub mod bindings;
pub mod capture;
pub mod clipboard;
pub mod cursor;
pub mod decorations;
pub mod dmabuf;
pub mod handlers;
pub mod idle;
pub mod input;
pub mod ipc;
pub mod layers;
pub mod layout;
pub mod lock;
pub mod remote;
pub mod screens;
pub mod settings;
pub mod shot;
pub mod state;
pub mod udev;
pub mod winit;
pub mod xwayland;

use std::{collections::BTreeMap, path::PathBuf};

use serde_json::{Map, Value};

use crate::{bindings::Bindings, decorations::Colors};

/// The default configuration: wm's own bindings. A config file's bindings
/// go over these (`null` unbinds a key).
pub const DEFAULT_CONFIG: &str = include_str!("../config.json");

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Backend {
    #[default]
    Winit,
    Drm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Alt,
    Super,
}


/// What the command line sets; it goes over the config file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Flags {
    pub backend: Option<Backend>,
    pub modifier: Option<Modifier>,
    pub seats: Option<usize>,
    pub command: Option<String>,
    /// `--config`: must exist. Without it, `$XDG_CONFIG_HOME/wm/config.json`
    /// is read if it is there.
    pub config: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub backend: Backend,
    /// The bindings' modifier (`Mod`): Super on real hardware; nested, Alt,
    /// as the host compositor keeps Super.
    pub modifier: Modifier,
    /// How many seats to start with.
    pub seats: usize,
    /// Run at startup (`-c`).
    pub command: Option<String>,
    /// Run at startup, from the config file's `startup`.
    pub startup: Vec<String>,
    /// Exact libinput device names mapped to wm seat indices; others use seat 0.
    pub devices: BTreeMap<String, usize>,
    pub bindings: Bindings,
    /// Seats' pointers and focus borders, and unfocused borders.
    pub colors: Colors,
    /// X11 apps through xwayland-satellite (if it is on PATH).
    pub xwayland: bool,
    /// Keyboard layout and repeat, touchpad and mouse options, focus
    /// following the mouse.
    pub input: settings::InputConfig,
    /// The combos whose bindings run while the session is locked.
    pub locked_bindings: std::collections::HashSet<(bindings::Mods, u32)>,
    /// Each monitor's mode, the screens' order and which are off.
    pub screens: Vec<screens::ScreenConfig>,
    /// The file read, if any, and the flags: `reload` reads it again.
    pub path: Option<PathBuf>,
    pub flags: Flags,
}

impl Default for Config {
    fn default() -> Self {
        Config::load(Flags { config: Some(PathBuf::new()), ..Default::default() }).expect("the default config")
    }
}

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Run the compositor.
    Run(Box<Config>),
    /// `wm msg <words…>`: send a command to a running one.
    Msg(Vec<String>),
}

pub const USAGE: &str = "usage: wm [--backend winit|drm] [--config FILE] [--mod alt|super] [--seats N] [-c|--command CMD]
       wm msg [seat N] COMMAND…   (see `wm msg help`)
The config file (JSON, default ~/.config/wm/config.json):
  {\"modifier\": \"super\", \"seats\": 2, \"startup\": [\"waybar\"],
   \"devices\": {\"USB Keyboard\": 1},
   \"bindings\": {\"Mod+Return\": \"spawn foot\", \"Mod+w\": null},
   \"input\": {\"keyboard\": {\"layout\": \"us\"}, \"touchpad\": {\"tap\": true}},
   \"screens\": [{\"match\": \"eDP-1\", \"mode\": \"1366x768@60\"}],
   \"locked_bindings\": [\"XF86AudioMute\"],
   \"xwayland\": true, \"colors\": {\"seats\": [\"#c1c5dd\", \"#eb5c5c\"], \"inactive\": \"#121318\"}}
Bindings go over wm's own (see config.json in its source); null unbinds.";

fn modifier(name: &str) -> Result<Modifier, String> {
    match name {
        "alt" => Ok(Modifier::Alt),
        "super" => Ok(Modifier::Super),
        other => Err(format!("unknown modifier {other}")),
    }
}

/// The config file wm reads without `--config`.
fn default_path() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(dir.join("wm").join("config.json")).filter(|path| path.is_file())
}

fn object(source: &str, name: &str) -> Result<Map<String, Value>, String> {
    match serde_json::from_str(source).map_err(|err| format!("{name}: {err}"))? {
        Value::Object(map) => Ok(map),
        _ => Err(format!("{name}: the config must be a JSON object")),
    }
}

impl Config {
    pub fn device_seat(&self, name: &str) -> usize {
        self.devices.get(name).copied().unwrap_or(0)
    }

    /// The defaults, then the config file, then the flags. An empty
    /// `--config` path reads no file.
    pub fn load(flags: Flags) -> Result<Config, String> {
        let path = match &flags.config {
            Some(path) if path.as_os_str().is_empty() => None,
            Some(path) => Some(path.clone()),
            None => default_path(),
        };
        let file = match &path {
            Some(path) => {
                let name = path.display().to_string();
                let source = std::fs::read_to_string(path).map_err(|err| format!("{name}: {err}"))?;
                object(&source, &name)?
            }
            None => Map::new(),
        };
        if let Some(key) = file.keys().find(|k| !["modifier", "seats", "startup", "devices", "bindings", "colors", "xwayland", "input", "screens", "locked_bindings"].contains(&k.as_str())) {
            return Err(format!("unknown config key {key:?}"));
        }

        let backend = flags.backend.unwrap_or_default();
        let modifier = match (flags.modifier, file.get("modifier")) {
            (Some(m), _) => m,
            (None, Some(Value::String(name))) => self::modifier(name)?,
            (None, Some(_)) => return Err("modifier must be \"alt\" or \"super\"".into()),
            (None, None) if backend == Backend::Drm => Modifier::Super,
            (None, None) => Modifier::Alt,
        };
        let seats = match (flags.seats, file.get("seats")) {
            (Some(n), _) => n,
            (None, Some(n)) => n.as_u64().filter(|&n| n >= 1).ok_or("seats must be a number from 1")? as usize,
            (None, None) => 2,
        };
        let startup = match file.get("startup") {
            None => Vec::new(),
            Some(Value::Array(list)) => list
                .iter()
                .map(|c| c.as_str().map(String::from).ok_or("startup must be a list of commands"))
                .collect::<Result<_, _>>()?,
            Some(_) => return Err("startup must be a list of commands".into()),
        };

        let mut devices = BTreeMap::new();
        match file.get("devices") {
            None => {}
            Some(Value::Object(map)) => {
                for (name, seat) in map {
                    let seat = seat.as_u64().and_then(|seat| usize::try_from(seat).ok())
                        .filter(|&seat| seat < seats)
                        .ok_or_else(|| format!("device {name:?} needs a seat index below {seats}"))?;
                    if name.trim().is_empty() {
                        return Err("device names must not be empty".into());
                    }
                    devices.insert(name.clone(), seat);
                }
            }
            Some(_) => return Err("devices must be an object".into()),
        }

        // wm's own bindings, then the file's over them.
        let mut specs: BTreeMap<String, String> = BTreeMap::new();
        let defaults = object(DEFAULT_CONFIG, "config.json")?;
        for source in [defaults.get("bindings"), file.get("bindings")].into_iter().flatten() {
            let Value::Object(map) = source else { return Err("bindings must be an object".into()) };
            for (combo, commands) in map {
                // Two spellings of one key: the later replaces the earlier.
                let key = bindings::combo_key(combo, modifier)?;
                specs.retain(|c, _| bindings::combo_key(c, modifier).ok() != Some(key));
                match commands {
                    Value::Null => {}
                    Value::String(commands) => {
                        specs.insert(combo.clone(), commands.clone());
                    }
                    _ => return Err(format!("binding {combo}: commands must be a string, or null")),
                }
            }
        }
        let bindings = Bindings::new(specs.iter().map(|(c, a)| (c.as_str(), a.as_str())), modifier)?;
        let mut locked_bindings = std::collections::HashSet::new();
        for source in [defaults.get("locked_bindings"), file.get("locked_bindings")].into_iter().flatten() {
            let list = source.as_array().ok_or("locked_bindings must be a list of combos")?;
            for combo in list {
                let combo = combo.as_str().ok_or("locked_bindings must be a list of combos")?;
                locked_bindings.insert(bindings::combo_key(combo, modifier)?);
            }
        }

        // wm's own colours, then the file's over them.
        let mut colors = Colors { seats: vec![[1.0; 4]], inactive: [0.0, 0.0, 0.0, 1.0] };
        for source in [defaults.get("colors"), file.get("colors")].into_iter().flatten() {
            colors = colors.merged(source)?;
        }

        // wm's own input settings, then the file's over them.
        let mut input = settings::InputConfig::default();
        for source in [defaults.get("input"), file.get("input")].into_iter().flatten() {
            input = input.merged(source)?;
        }

        let xwayland = match file.get("xwayland") {
            None => true,
            Some(Value::Bool(on)) => *on,
            Some(_) => return Err("xwayland must be true or false".into()),
        };

        let screens = match file.get("screens") {
            None => Vec::new(),
            Some(value) => screens::parse(value)?,
        };

        Ok(Config { backend, modifier, seats, command: flags.command.clone(), startup, devices, bindings, colors, xwayland, input, locked_bindings, screens, path, flags })
    }

    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
        let mut args = args.into_iter().peekable();
        if args.peek().map(String::as_str) == Some("msg") {
            return Ok(Command::Msg(args.skip(1).collect()));
        }
        let mut flags = Flags::default();
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value\n{USAGE}"));
            match arg.as_str() {
                "--backend" => flags.backend = Some(match value()?.as_str() {
                    "winit" => Backend::Winit,
                    "drm" => Backend::Drm,
                    other => return Err(format!("unknown backend {other}\n{USAGE}")),
                }),
                "--config" => flags.config = Some(PathBuf::from(value()?)).filter(|p| !p.as_os_str().is_empty()),
                "--mod" => flags.modifier = Some(modifier(&value()?).map_err(|err| format!("{err}\n{USAGE}"))?),
                "--seats" => match value()?.parse() {
                    Ok(n) if n >= 1 => flags.seats = Some(n),
                    _ => return Err(format!("--seats takes a number from 1\n{USAGE}")),
                },
                "-c" | "--command" => flags.command = Some(value()?),
                "-h" | "--help" => return Err(USAGE.into()),
                other => return Err(format!("unknown option {other}\n{USAGE}")),
            }
        }
        Ok(Command::Run(Box::new(Config::load(flags)?)))
    }
}
