//! wm: a scrolling Wayland compositor with several seats and animations per
//! workspace. `main.rs` only starts it; everything lives here so the tests in
//! `tests/` can reach it. The command-line options are below.

pub mod dmabuf;
pub mod handlers;
pub mod input;
pub mod ipc;
pub mod layout;
pub mod state;
pub mod udev;
pub mod winit;

use smithay::input::keyboard::ModifiersState;
use std::collections::BTreeMap;

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

impl Modifier {
    pub fn held(self, mods: &ModifiersState) -> bool {
        match self {
            Modifier::Alt => mods.alt,
            Modifier::Super => mods.logo,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub backend: Backend,
    /// The bindings' modifier. Alt by default, as a host compositor keeps Super.
    pub modifier: Modifier,
    /// How many seats to start with.
    pub seats: usize,
    /// Run at startup.
    pub command: Option<String>,
    /// Exact libinput device names mapped to wm seat indices; others use seat 0.
    pub devices: BTreeMap<String, usize>,
}

impl Default for Config {
    fn default() -> Self {
        Config { backend: Backend::Winit, modifier: Modifier::Alt, seats: 2, command: None, devices: BTreeMap::new() }
    }
}

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Run the compositor.
    Run(Config),
    /// `wm msg <words…>`: send a command to a running one.
    Msg(Vec<String>),
}

pub const USAGE: &str = "usage: wm [--backend winit|drm] [--config FILE] [--mod alt|super] [--seats N] [-c|--command CMD]
       wm msg [seat N] COMMAND…   (see `wm msg help`)";

impl Config {
    pub fn device_seat(&self, name: &str) -> usize {
        self.devices.get(name).copied().unwrap_or(0)
    }

    /// Minimal device configuration: {"devices":{"USB Keyboard":1}}.
    /// Names must match libinput exactly and indices must exist in --seats.
    fn read_devices(&mut self, path: &str) -> Result<(), String> {
        let source = std::fs::read_to_string(path).map_err(|err| format!("{path}: {err}"))?;
        let json: serde_json::Value = serde_json::from_str(&source).map_err(|err| format!("{path}: {err}"))?;
        let root = json.as_object().ok_or("config must be an object")?;
        if root.keys().any(|key| key != "devices") {
            return Err("config only supports devices".into());
        }
        let Some(devices) = root.get("devices") else { return Ok(()) };
        let devices = devices.as_object().ok_or("devices must be an object")?;
        for (name, seat) in devices {
            let seat = seat.as_u64().and_then(|seat| usize::try_from(seat).ok())
                .filter(|&seat| seat < self.seats)
                .ok_or_else(|| format!("device {name:?} needs a seat index below {}", self.seats))?;
            if name.trim().is_empty() {
                return Err("device names must not be empty".into());
            }
            self.devices.insert(name.clone(), seat);
        }
        Ok(())
    }

    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
        let mut args = args.into_iter().peekable();
        if args.peek().map(String::as_str) == Some("msg") {
            return Ok(Command::Msg(args.skip(1).collect()));
        }
        let mut config = Config::default();
        let mut config_path = None;
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value\n{USAGE}"));
            match arg.as_str() {
                "--backend" => config.backend = match value()?.as_str() {
                    "winit" => Backend::Winit,
                    "drm" => Backend::Drm,
                    other => return Err(format!("unknown backend {other}\n{USAGE}")),
                },
                "--config" => config_path = Some(value()?),
                "--mod" => {
                    config.modifier = match value()?.as_str() {
                        "alt" => Modifier::Alt,
                        "super" => Modifier::Super,
                        other => return Err(format!("unknown modifier {other}\n{USAGE}")),
                    }
                }
                "--seats" => match value()?.parse() {
                    Ok(n) if n >= 1 => config.seats = n,
                    _ => return Err(format!("--seats takes a number from 1\n{USAGE}")),
                },
                "-c" | "--command" => config.command = Some(value()?),
                "-h" | "--help" => return Err(USAGE.into()),
                other => return Err(format!("unknown option {other}\n{USAGE}")),
            }
        }
        if let Some(path) = config_path {
            config.read_devices(&path)?;
        }
        Ok(Command::Run(config))
    }
}
