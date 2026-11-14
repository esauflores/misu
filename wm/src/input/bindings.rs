//! Key bindings: a key with modifiers held runs commands for the seat whose
//! keyboard it is.
//!
//! In the config file they are written as `"Mod+Shift+1": "move-to-workspace
//! 1; workspace 1"`: modifiers and a key name (an xkb keysym name, any case)
//! joined by `+`, and commands in `wm msg`'s words, `;` between them. `Mod`
//! is the configured modifier (`--mod`, `"modifier"`).

use std::collections::HashMap;

use smithay::input::keyboard::{Keysym, ModifiersState, xkb};

use crate::{
    Modifier,
    input::Action,
    ipc::{self, Command},
};

/// The modifiers a binding needs, exactly: others held don't match.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
}

impl Mods {
    pub fn held(state: &ModifiersState) -> Self {
        Self { ctrl: state.ctrl, alt: state.alt, shift: state.shift, logo: state.logo }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bindings(HashMap<(Mods, u32), Vec<Action>>);

impl Bindings {
    /// Compile `"combo": "commands"` pairs, with `Mod` meaning `modifier`.
    pub fn new<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>, modifier: Modifier) -> Result<Self, String> {
        let mut map = HashMap::new();
        for (combo, commands) in pairs {
            let key = combo_key(combo, modifier)?;
            let actions = actions(commands).map_err(|err| format!("binding {combo}: {err}"))?;
            map.insert(key, actions);
        }
        Ok(Self(map))
    }

    /// What `sym` (as the keyboard gives it, unshifted) runs with `mods` held.
    pub fn get(&self, mods: &ModifiersState, sym: Keysym) -> Option<&[Action]> {
        self.0.get(&pressed(mods, sym)).map(Vec::as_slice)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The combo `sym` pressed with `mods` held is.
pub fn pressed(mods: &ModifiersState, sym: Keysym) -> (Mods, u32) {
    (Mods::held(mods), lower(sym.raw()))
}

/// `Mod+Shift+Return` as the modifiers and the keysym it matches.
pub fn combo_key(combo: &str, modifier: Modifier) -> Result<(Mods, u32), String> {
    let parts: Vec<&str> = combo.split('+').map(str::trim).collect();
    let (key, held) = parts.split_last().filter(|(k, _)| !k.is_empty()).ok_or(format!("binding {combo:?}: no key"))?;
    let mut mods = Mods::default();
    for name in held {
        let flag = match name.to_ascii_lowercase().as_str() {
            "mod" => match modifier {
                Modifier::Alt => &mut mods.alt,
                Modifier::Super => &mut mods.logo,
            },
            "super" | "logo" | "mod4" => &mut mods.logo,
            "alt" | "mod1" => &mut mods.alt,
            "ctrl" | "control" => &mut mods.ctrl,
            "shift" => &mut mods.shift,
            other => return Err(format!("binding {combo:?}: unknown modifier {other}")),
        };
        *flag = true;
    }
    let sym = xkb::keysym_from_name(key, xkb::KEYSYM_CASE_INSENSITIVE);
    if sym.raw() == 0 {
        return Err(format!("binding {combo:?}: unknown key {key}"));
    }
    Ok((mods, lower(sym.raw())))
}

/// `focus left; workspace 2`: commands for the pressing seat, so no `seat N`,
/// and only ones that act (not `tree`, `subscribe`, …).
pub fn actions(commands: &str) -> Result<Vec<Action>, String> {
    let mut actions = Vec::new();
    for command in commands.split(';').map(str::trim).filter(|c| !c.is_empty()) {
        let request = ipc::parse(command)?;
        match (request.seat, request.command) {
            (None, Command::Action(action)) => actions.push(action),
            (Some(_), _) => return Err(format!("{command}: a binding acts for the seat pressing it")),
            _ => return Err(format!("{command}: not a command a key can run")),
        }
    }
    if actions.is_empty() {
        return Err("no command".into());
    }
    Ok(actions)
}

/// Letters match whatever their case: the keyboard gives them unshifted.
fn lower(sym: u32) -> u32 {
    if (u32::from(b'A')..=u32::from(b'Z')).contains(&sym) { sym + 0x20 } else { sym }
}
