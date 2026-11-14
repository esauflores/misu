//! The scrolling layout: each workspace is a row of columns, wider than the
//! screen. Every seat has its own view of a workspace (how far it is
//! scrolled), so a seat moving around never scrolls anyone else's.
//!
//! Whether a change animates is the caller's decision (`animate`), made per
//! seat: the seat's own setting, and whether anyone is looking. The layout
//! only keeps animations smooth: a change mid-way carries on from where it is.
//!
//! It is generic over the window type (`Tile`), so tests can use fake windows.
//!
//! An `Anim` never restarts: retargeting it while it moves starts the new
//! curve from where it is now, so a burst of changes (focus from several
//! seats, windows opening) reads as one smooth motion instead of a series of
//! jumps back to an old start.

use std::time::{Duration, Instant};

use smithay::{
    desktop::Window,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Point, Rectangle, Size},
    wayland::shell::xdg::ToplevelStateSet,
};

pub const GAP: i32 = 12;
/// A new column's width, as a fraction of the screen's: all of it, one
/// window per screen, scrolled between.
pub const DEFAULT_WIDTH: f64 = 1.0;

/// Whether `app_id` is a Chromium extension's window (Brave's Bitwarden
/// popup): `<browser>-<id>-<profile>`, the id 32 letters from a to p.
pub fn extension_window(app_id: &str) -> bool {
    app_id.split('-').any(|part| part.len() == 32 && part.bytes().all(|b| (b'a'..=b'p').contains(&b)))
}

/// Whether a window's size range (`min`, `max`; 0 for no limit) is one fixed
/// size: a dialog (polkit's prompt), floated even without a parent.
pub fn fixed_size(min: Size<i32, Logical>, max: Size<i32, Logical>) -> bool {
    min.w > 0 && min.h > 0 && min == max
}

/// A dialog's size: 3/4 of the screen's area each way, within the size range
/// it allows (`min`, `max`; 0 for no limit), so a fixed-size one keeps its own.
pub fn dialog_size(area: Size<i32, Logical>, min: Size<i32, Logical>, max: Size<i32, Logical>) -> Size<i32, Logical> {
    let fit = |want: i32, min: i32, max: i32| if max > 0 { want.min(max) } else { want }.max(min);
    (fit(area.w * 3 / 4, min.w, max.w), fit(area.h * 3 / 4, min.h, max.h)).into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
}

/// What the layout needs from a window.
pub trait Tile: Clone + PartialEq {
    /// Ask the window to take this size, full screen or not.
    fn configure(&self, size: Size<i32, Logical>, fullscreen: bool);
    /// The size it has: a floating window keeps its own.
    fn size(&self) -> Size<i32, Logical>;
}

impl Tile for Window {
    fn configure(&self, size: Size<i32, Logical>, fullscreen: bool) {
        if let Some(toplevel) = self.toplevel() {
            toplevel.with_pending_state(|s| {
                s.size = Some(size);
                set_fullscreen_state(&mut s.states, fullscreen);
            });
            // Before its first commit the window gets this size with its
            // initial configure (see `commit` in handlers.rs).
            if toplevel.is_initial_configure_sent() {
                toplevel.send_pending_configure();
            }
        }
    }

    fn size(&self) -> Size<i32, Logical> {
        self.geometry().size
    }
}

/// A toplevel's full-screen state, on or off.
pub fn set_fullscreen_state(states: &mut ToplevelStateSet, on: bool) {
    if on { states.set(xdg_toplevel::State::Fullscreen) } else { states.unset(xdg_toplevel::State::Fullscreen) };
}

struct Column<W> {
    window: W,
    width: f64,
    /// It covers the whole screen, bar and gaps too, when a view has it
    /// at the screen's left edge.
    fullscreen: bool,
    x: Anim,
}

/// A window an app opened for another of its windows (a dialog): it floats
/// centred over that one, its parent, wherever a seat's view has it, at the
/// size it picks. Without a parent here, centred on the screen.
struct Float<W> {
    window: W,
    parent: Option<W>,
}

pub struct Workspace<W: Tile = Window> {
    /// The part of its screen it is laid out in: all of it but what layer
    /// surfaces (a bar) keep for themselves, in the screen's coordinates.
    area: Rectangle<i32, Logical>,
    /// The whole screen's size, for full-screen windows.
    screen: Size<i32, Logical>,
    columns: Vec<Column<W>>,
    /// Over the columns, the last on top; a parent before its children.
    floats: Vec<Float<W>>,
    /// Each seat's scroll position, by seat index.
    views: Vec<Anim>,
}

impl<W: Tile> Workspace<W> {
    pub fn new(size: Size<i32, Logical>) -> Self {
        Self { area: Rectangle::from_size(size), screen: size, columns: Vec::new(), floats: Vec::new(), views: Vec::new() }
    }

    pub fn size(&self) -> Size<i32, Logical> {
        self.area.size
    }

    /// Lay out again in `area` of a screen of size `screen`.
    pub fn set_area(&mut self, area: Rectangle<i32, Logical>, screen: Size<i32, Logical>, now: Instant, animate: bool) {
        if area != self.area || screen != self.screen {
            self.area = area;
            self.screen = screen;
            self.arrange(now, animate);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty() && self.floats.is_empty()
    }

    /// The columns' windows, then the floating ones.
    pub fn windows(&self) -> impl Iterator<Item = &W> {
        self.columns.iter().map(|c| &c.window).chain(self.floats.iter().map(|f| &f.window))
    }

    pub fn contains(&self, window: &W) -> bool {
        self.windows().any(|w| w == window)
    }

    pub fn is_floating(&self, window: &W) -> bool {
        self.floats.iter().any(|f| &f.window == window)
    }

    pub fn is_fullscreen(&self, window: &W) -> bool {
        self.columns.iter().any(|c| &c.window == window && c.fullscreen)
    }

    /// Make `window`'s column full screen or not (`None`: the other way);
    /// whether that changed anything. Floating windows stay as they are.
    pub fn set_fullscreen(&mut self, window: &W, on: Option<bool>, now: Instant, animate: bool) -> bool {
        let Some(index) = self.index_of(window) else { return false };
        let column = &mut self.columns[index];
        let on = on.unwrap_or(!column.fullscreen);
        if column.fullscreen == on {
            return false;
        }
        column.fullscreen = on;
        self.arrange(now, animate);
        true
    }

    /// Whether `seat`'s view at `now` has a full-screen window covering the
    /// screen: what layer surfaces over windows (a bar) give way to.
    pub fn covered(&self, seat: usize, now: Instant) -> bool {
        let view = self.view(seat, now);
        self.columns.iter().any(|c| c.fullscreen && (c.x.value(now) - view).round() == 0.0)
    }

    fn index_of(&self, window: &W) -> Option<usize> {
        self.columns.iter().position(|c| &c.window == window)
    }

    /// The column `window` is, or (floating) the one its parents float over.
    fn column_of(&self, window: &W) -> Option<usize> {
        let mut window = window.clone();
        for _ in 0..=self.floats.len() {
            if let Some(index) = self.index_of(&window) {
                return Some(index);
            }
            window = self.floats.iter().find(|f| f.window == window)?.parent.clone()?;
        }
        None
    }

    /// Add `window` floating over `parent` (if it is here).
    pub fn add_float(&mut self, window: W, parent: Option<&W>) {
        let parent = parent.filter(|p| self.contains(p)).cloned();
        self.floats.push(Float { window, parent });
    }

    fn view_mut(&mut self, seat: usize) -> &mut Anim {
        if self.views.len() <= seat {
            self.views.resize(seat + 1, Anim::new(0.0));
        }
        &mut self.views[seat]
    }

    /// Where `seat`'s view is headed.
    pub fn view_target(&self, seat: usize) -> f64 {
        self.views.get(seat).map_or(0.0, Anim::target)
    }

    /// `seat`'s view at `now`.
    pub fn view(&self, seat: usize, now: Instant) -> f64 {
        self.views.get(seat).map_or(0.0, |v| v.value(now))
    }

    /// The size a new column's window gets.
    pub fn new_column_size(&self) -> Size<i32, Logical> {
        (self.column_width(self.columns.len()), (self.area.size.h - 2 * GAP).max(1)).into()
    }

    /// Add `window` as a new column after `after` (or at the end).
    pub fn add(&mut self, window: W, after: Option<&W>, now: Instant, animate: bool) {
        let index = after.and_then(|w| self.index_of(w)).map_or(self.columns.len(), |i| i + 1);
        // It appears in its place; the columns after it slide over.
        let x = self.column_x(index);
        self.columns.insert(index, Column { window, width: DEFAULT_WIDTH, fullscreen: false, x: Anim::new(x) });
        self.arrange(now, animate);
    }

    /// Remove `window`; returns the window that should take its focus: a
    /// floating one's parent, a column's neighbour. Windows floating over it
    /// stay, centred on the screen.
    pub fn remove(&mut self, window: &W, now: Instant, animate: bool) -> Option<W> {
        for float in &mut self.floats {
            if float.parent.as_ref() == Some(window) {
                float.parent = None;
            }
        }
        if let Some(i) = self.floats.iter().position(|f| &f.window == window) {
            let float = self.floats.remove(i);
            return float.parent.or_else(|| self.columns.first().map(|c| c.window.clone()));
        }
        let index = self.index_of(window)?;
        self.columns.remove(index);
        self.arrange(now, animate);
        let next = index.min(self.columns.len().checked_sub(1)?);
        Some(self.columns[next].window.clone())
    }

    /// The column beside `window`'s (a floating one's: its parent's).
    pub fn neighbour(&self, window: &W, dir: Direction) -> Option<W> {
        let index = self.column_of(window)?;
        let index = match dir {
            Direction::Left => index.checked_sub(1)?,
            Direction::Right => index + 1,
        };
        self.columns.get(index).map(|c| c.window.clone())
    }

    pub fn first(&self) -> Option<W> {
        self.columns.first().map(|c| c.window.clone())
    }

    pub fn last(&self) -> Option<W> {
        self.columns.last().map(|c| c.window.clone())
    }

    pub fn move_column(&mut self, window: &W, dir: Direction, now: Instant, animate: bool) {
        let Some(index) = self.index_of(window) else { return };
        let other = match dir {
            Direction::Left if index > 0 => index - 1,
            Direction::Right if index + 1 < self.columns.len() => index + 1,
            _ => return,
        };
        self.columns.swap(index, other);
        self.arrange(now, animate);
    }

    /// Scroll `seat`'s view just enough to show `window` whole (a floating
    /// one: the column it floats over).
    pub fn reveal(&mut self, seat: usize, window: &W, now: Instant, animate: bool) {
        let Some(index) = self.column_of(window) else { return };
        let x = self.columns[index].x.target();
        let (w, gap, screen) = (self.column_width(index) as f64, GAP as f64, self.area.size.w as f64);
        let view = self.view_target(seat);
        let target = if self.columns[index].fullscreen {
            // Exactly at the screen's edge, to cover it.
            x
        } else if x - gap < view {
            x - gap
        } else if x + w + gap > view + screen {
            x + w + gap - screen
        } else {
            return;
        };
        self.view_mut(seat).set(target.max(0.0), now, animate);
    }

    /// Lay the columns out again and tell the windows their sizes.
    fn arrange(&mut self, now: Instant, animate: bool) {
        for index in 0..self.columns.len() {
            let x = self.column_x(index);
            let size = self.window_size(index);
            let column = &mut self.columns[index];
            column.x.set(x, now, animate);
            column.window.configure(size, column.fullscreen);
        }
        // Don't leave a view past the last column.
        let end = (self.column_x(self.columns.len()) - self.area.size.w as f64).max(0.0);
        for view in &mut self.views {
            if view.target() > end {
                view.set(end, now, animate);
            }
        }
    }

    /// Stop every animation where it's headed.
    pub fn finish(&mut self) {
        self.views.iter_mut().for_each(Anim::finish);
        self.columns.iter_mut().for_each(|c| c.x.finish());
    }

    /// Stop `seat`'s view where it's headed.
    pub fn finish_view(&mut self, seat: usize) {
        if let Some(view) = self.views.get_mut(seat) {
            view.finish();
        }
    }

    /// Where each window is at `now`, as `seat` sees it on screen: the
    /// columns, then the floating windows over them.
    pub fn positions(&self, seat: usize, now: Instant) -> impl Iterator<Item = (&W, Point<i32, Logical>)> {
        let view = self.view(seat, now);
        self.place(|c| c.x.value(now) - view).into_iter().map(|(w, r)| (w, r.loc))
    }

    /// Where each window is headed on the screen, before any view.
    pub fn targets(&self) -> impl Iterator<Item = (&W, Rectangle<i32, Logical>)> {
        self.place(|c| c.x.target()).into_iter()
    }

    /// Every window's rectangle, columns at `x` along the screen.
    fn place(&self, x: impl Fn(&Column<W>) -> f64) -> Vec<(&W, Rectangle<i32, Logical>)> {
        let origin = self.area.loc;
        let mut placed: Vec<(&W, Rectangle<i32, Logical>)> = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let x = x(c).round() as i32;
                // Full screen: from the screen's corner, whatever the bar
                // keeps (its column has the bar's room before it), so a view
                // with it at the edge has it all.
                let loc = if c.fullscreen { Point::from((x, 0)) } else { origin + Point::from((x, GAP)) };
                (&c.window, Rectangle::new(loc, self.window_size(i)))
            })
            .collect();
        for float in &self.floats {
            let under = float.parent.as_ref().and_then(|p| placed.iter().find(|(w, _)| *w == p)).map(|(_, r)| *r);
            let under = under.unwrap_or(self.area);
            let size = float.window.size();
            let loc = under.loc + Point::from(((under.size.w - size.w) / 2, (under.size.h - size.h) / 2));
            placed.push((&float.window, Rectangle::new(loc, size)));
        }
        placed
    }

    /// Whether the columns, or `seat`'s view, are still moving.
    pub fn is_animating(&self, seat: usize, now: Instant) -> bool {
        self.views.get(seat).is_some_and(|v| !v.is_done(now)) || self.columns.iter().any(|c| !c.x.is_done(now))
    }

    /// `fraction` of the screen, less its share of the gaps: 1/fraction
    /// columns and the gaps around them fill the screen exactly.
    /// A full-screen column's window is the whole screen, the bar's room
    /// (`lead`) included: its column is the rest, with that room before it
    /// and ordinary gaps around, so it overlaps no other column.
    fn column_width(&self, index: usize) -> i32 {
        if self.columns.get(index).is_some_and(|c| c.fullscreen) {
            return (self.screen.w - self.lead(index) as i32).max(1);
        }
        let fraction = self.columns.get(index).map_or(DEFAULT_WIDTH, |c| c.width);
        ((self.area.size.w as f64 * fraction - GAP as f64 * (1.0 + fraction)) as i32).max(1)
    }

    /// The size column `index`'s window gets.
    fn window_size(&self, index: usize) -> Size<i32, Logical> {
        if self.columns[index].fullscreen {
            return (self.screen.w.max(1), self.screen.h.max(1)).into();
        }
        (self.column_width(index), (self.area.size.h - 2 * GAP).max(1)).into()
    }

    fn column_x(&self, index: usize) -> f64 {
        (0..index).fold(GAP as f64, |x, i| x + self.lead(i) + (self.column_width(i) + GAP) as f64) + self.lead(index)
    }

    /// The room a full-screen column keeps before it: what the bar takes at
    /// the screen's left, which its window covers.
    fn lead(&self, index: usize) -> f64 {
        if self.columns.get(index).is_some_and(|c| c.fullscreen) { self.area.loc.x as f64 } else { 0.0 }
    }
}

/// How long an animation takes.
pub const DURATION: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
pub struct Anim {
    from: f64,
    to: f64,
    start: Instant,
    duration: Duration,
}

impl Anim {
    pub fn new(value: f64) -> Self {
        Self { from: value, to: value, start: Instant::now(), duration: Duration::ZERO }
    }

    /// The value at `now`.
    pub fn value(&self, now: Instant) -> f64 {
        // Ease out, cubic.
        self.from + (self.to - self.from) * (1.0 - (1.0 - self.progress(now)).powi(3))
    }

    pub fn target(&self) -> f64 {
        self.to
    }

    pub fn is_done(&self, now: Instant) -> bool {
        self.progress(now) >= 1.0
    }

    /// Head for `to`, from the current value. With `animate` false, jump there.
    pub fn set(&mut self, to: f64, now: Instant, animate: bool) {
        if to == self.to {
            return;
        }
        self.from = if animate { self.value(now) } else { to };
        self.to = to;
        self.start = now;
        self.duration = if animate { DURATION } else { Duration::ZERO };
    }

    /// Jump to where it's headed (animations were turned off).
    pub fn finish(&mut self) {
        self.from = self.to;
        self.duration = Duration::ZERO;
    }

    fn progress(&self, now: Instant) -> f64 {
        if self.duration.is_zero() {
            return 1.0;
        }
        (now.saturating_duration_since(self.start).as_secs_f64() / self.duration.as_secs_f64()).min(1.0)
    }
}
