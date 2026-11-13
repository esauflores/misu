//! wm: a scrolling Wayland compositor with several seats and animations per
//! workspace. `main.rs` only starts it; everything lives here so the tests in
//! `tests/` can reach it. The command-line options are below; a config file
//! comes later.

pub mod handlers;
pub mod input;
pub mod ipc;
pub mod layout;
pub mod state;
pub mod winit;

use smithay::input::keyboard::ModifiersState;

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
    /// The bindings' modifier. Alt by default, as a host compositor keeps Super.
    pub modifier: Modifier,
    /// How many seats to start with.
    pub seats: usize,
    /// Run at startup.
    pub command: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config { modifier: Modifier::Alt, seats: 2, command: None }
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

pub const USAGE: &str = "usage: wm [--mod alt|super] [--seats N] [-c|--command CMD]
       wm msg [seat N] COMMAND…   (see `wm msg help`)";

impl Config {
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
        let mut args = args.into_iter().peekable();
        if args.peek().map(String::as_str) == Some("msg") {
            return Ok(Command::Msg(args.skip(1).collect()));
        }
        let mut config = Config::default();
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value\n{USAGE}"));
            match arg.as_str() {
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
        Ok(Command::Run(config))
    }
}
