//! Screen settings, the config file's `screens`: each monitor's mode, the
//! order screens go in from left to right, and which are off.
//!
//! ```json
//! "screens": [
//!   {"match": "HDMI-A-1", "mode": "2560x1440@144"},
//!   {"match": "eDP-1", "mode": "1366x768@60", "off": false}
//! ]
//! ```
//!
//! `match` is a connector (`eDP-1`, or with the GPU, `card1-eDP-1`) or a
//! monitor's identity (make, model and serial, as `wm msg tree` shows it).
//! The first entry that matches a monitor is its. Screens with an entry go
//! in the list's order, left to right, then the others in plug order. A
//! mode is `WxH` (the highest refresh) or `WxH@Hz` (the nearest); without
//! one, or with none matching, a monitor gets its preferred mode.
//!
//! A laptop's panel (`eDP`, `LVDS`, `DSI`) is off while its lid is closed.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScreenConfig {
    pub matches: String,
    /// Width, height, and refresh in mHz if given.
    pub mode: Option<(i32, i32, Option<i32>)>,
    pub off: bool,
}

/// The config's `screens`, in order.
pub fn parse(value: &Value) -> Result<Vec<ScreenConfig>, String> {
    let list = value.as_array().ok_or("screens must be a list")?;
    list.iter()
        .map(|entry| {
            let map = entry.as_object().ok_or("a screen is an object")?;
            let mut screen = ScreenConfig::default();
            for (key, value) in map {
                match key.as_str() {
                    "match" => screen.matches = value.as_str().filter(|s| !s.is_empty()).ok_or("screen match must be a string")?.into(),
                    "mode" => screen.mode = Some(mode(value.as_str().ok_or("screen mode must be a string")?)?),
                    "off" => screen.off = value.as_bool().ok_or("screen off must be true or false")?,
                    _ => return Err(format!("screen: unknown key {key:?}")),
                }
            }
            if screen.matches.is_empty() {
                return Err("a screen needs a match".into());
            }
            Ok(screen)
        })
        .collect()
}

/// `"WxH"` or `"WxH@Hz"` (Hz may have decimals).
fn mode(text: &str) -> Result<(i32, i32, Option<i32>), String> {
    let bad = || format!("bad mode {text:?}: it is WxH or WxH@Hz");
    let (size, refresh) = match text.split_once('@') {
        Some((size, hz)) => (size, Some(hz.trim_end_matches("Hz").parse::<f64>().map_err(|_| bad())?)),
        None => (text, None),
    };
    let (w, h) = size.split_once('x').ok_or_else(bad)?;
    let (w, h) = (w.parse::<i32>().map_err(|_| bad())?, h.parse::<i32>().map_err(|_| bad())?);
    if w <= 0 || h <= 0 || refresh.is_some_and(|r| r <= 0.0) {
        return Err(bad());
    }
    Ok((w, h, refresh.map(|r| (r * 1000.0).round() as i32)))
}

/// Whether `pattern` names the monitor at output `name` (`card1-eDP-1`) with
/// `identity`.
pub fn matches(pattern: &str, name: &str, identity: &str) -> bool {
    pattern == name || name.ends_with(&format!("-{pattern}")) || pattern == identity
}

/// The index of the entry for a monitor, if any.
pub fn find(list: &[ScreenConfig], name: &str, identity: &str) -> Option<usize> {
    list.iter().position(|s| matches(&s.matches, name, identity))
}

/// Whether output `name` is a laptop's built-in panel.
pub fn is_internal(name: &str) -> bool {
    ["eDP", "LVDS", "DSI"].iter().any(|kind| name.contains(&format!("-{kind}-")) || name.starts_with(&format!("{kind}-")))
}

/// Which of `modes` (width, height, refresh mHz) to use for `wanted`: the
/// index, or None to keep the preferred one.
pub fn choose_mode(modes: &[(i32, i32, i32)], wanted: (i32, i32, Option<i32>)) -> Option<usize> {
    let (w, h, refresh) = wanted;
    let sized = modes.iter().enumerate().filter(|(_, m)| m.0 == w && m.1 == h);
    match refresh {
        Some(r) => sized.min_by_key(|(_, m)| (m.2 - r).abs()).map(|(i, _)| i),
        None => sized.max_by_key(|(_, m)| m.2).map(|(i, _)| i),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_screens() {
        let list = parse(&json!([{"match": "eDP-1", "mode": "1366x768@60.059Hz"}, {"match": "HDMI-A-1", "off": true}])).unwrap();
        assert_eq!(list[0].mode, Some((1366, 768, Some(60_059))));
        assert!(list[1].off);
        assert_eq!(parse(&json!([{"match": "x", "mode": "1366x768"}])).unwrap()[0].mode, Some((1366, 768, None)));
        assert!(parse(&json!([{"mode": "1x1"}])).is_err());
        assert!(parse(&json!([{"match": "x", "mode": "big"}])).is_err());
        assert!(parse(&json!([{"match": "x", "scale": 2}])).is_err());
    }

    #[test]
    fn matches_connectors_and_identities() {
        assert!(matches("eDP-1", "card1-eDP-1", "BOE 0x0731"));
        assert!(matches("card1-eDP-1", "card1-eDP-1", ""));
        assert!(matches("BOE 0x0731", "card1-eDP-1", "BOE 0x0731"));
        assert!(!matches("DP-1", "card1-eDP-1", ""));
        assert!(!matches("eDP", "card1-eDP-1", ""));
        assert!(is_internal("card1-eDP-1") && is_internal("LVDS-1") && !is_internal("card1-DP-2"));
    }

    #[test]
    fn chooses_modes() {
        let modes = [(1920, 1080, 60_000), (1920, 1080, 144_000), (1366, 768, 60_059), (1366, 768, 40_000)];
        assert_eq!(choose_mode(&modes, (1920, 1080, None)), Some(1));
        assert_eq!(choose_mode(&modes, (1366, 768, Some(60_000))), Some(2));
        assert_eq!(choose_mode(&modes, (800, 600, None)), None);
    }
}
