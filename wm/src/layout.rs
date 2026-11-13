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
    utils::{Logical, Point, Rectangle, Size},
};

pub const GAP: i32 = 12;
/// A new column's width, as a fraction of the screen's.
pub const DEFAULT_WIDTH: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
}

/// What the layout needs from a window.
pub trait Tile: Clone + PartialEq {
    /// Ask the window to take this size.
    fn configure(&self, size: Size<i32, Logical>);
}

impl Tile for Window {
    fn configure(&self, size: Size<i32, Logical>) {
        if let Some(toplevel) = self.toplevel() {
            toplevel.with_pending_state(|s| s.size = Some(size));
            // Before its first commit the window gets this size with its
            // initial configure (see `commit` in handlers.rs).
            if toplevel.is_initial_configure_sent() {
                toplevel.send_pending_configure();
            }
        }
    }
}

struct Column<W> {
    window: W,
    width: f64,
    x: Anim,
}

pub struct Workspace<W: Tile = Window> {
    /// The screen size it is laid out for.
    size: Size<i32, Logical>,
    columns: Vec<Column<W>>,
    /// Each seat's scroll position, by seat index.
    views: Vec<Anim>,
}

impl<W: Tile> Workspace<W> {
    pub fn new(size: Size<i32, Logical>) -> Self {
        Self { size, columns: Vec::new(), views: Vec::new() }
    }

    pub fn size(&self) -> Size<i32, Logical> {
        self.size
    }

    /// Lay out again for a screen of this size.
    pub fn resize(&mut self, size: Size<i32, Logical>, now: Instant, animate: bool) {
        self.size = size;
        self.arrange(now, animate);
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn windows(&self) -> impl Iterator<Item = &W> {
        self.columns.iter().map(|c| &c.window)
    }

    pub fn contains(&self, window: &W) -> bool {
        self.index_of(window).is_some()
    }

    fn index_of(&self, window: &W) -> Option<usize> {
        self.columns.iter().position(|c| &c.window == window)
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
        (self.column_width(self.columns.len()), (self.size.h - 2 * GAP).max(1)).into()
    }

    /// Add `window` as a new column after `after` (or at the end).
    pub fn add(&mut self, window: W, after: Option<&W>, now: Instant, animate: bool) {
        let index = after.and_then(|w| self.index_of(w)).map_or(self.columns.len(), |i| i + 1);
        // It appears in its place; the columns after it slide over.
        let x = self.column_x(index);
        self.columns.insert(index, Column { window, width: DEFAULT_WIDTH, x: Anim::new(x) });
        self.arrange(now, animate);
    }

    /// Remove `window`; returns the window that should take its focus.
    pub fn remove(&mut self, window: &W, now: Instant, animate: bool) -> Option<W> {
        let index = self.index_of(window)?;
        self.columns.remove(index);
        self.arrange(now, animate);
        let next = index.min(self.columns.len().checked_sub(1)?);
        Some(self.columns[next].window.clone())
    }

    pub fn neighbour(&self, window: &W, dir: Direction) -> Option<W> {
        let index = self.index_of(window)?;
        let index = match dir {
            Direction::Left => index.checked_sub(1)?,
            Direction::Right => index + 1,
        };
        self.columns.get(index).map(|c| c.window.clone())
    }

    pub fn first(&self) -> Option<W> {
        self.columns.first().map(|c| c.window.clone())
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

    /// Scroll `seat`'s view just enough to show `window` whole.
    pub fn reveal(&mut self, seat: usize, window: &W, now: Instant, animate: bool) {
        let Some(index) = self.index_of(window) else { return };
        let x = self.columns[index].x.target();
        let (w, gap, screen) = (self.column_width(index) as f64, GAP as f64, self.size.w as f64);
        let view = self.view_target(seat);
        let target = if x - gap < view {
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
        let height = (self.size.h - 2 * GAP).max(1);
        for index in 0..self.columns.len() {
            let (x, width) = (self.column_x(index), self.column_width(index));
            let column = &mut self.columns[index];
            column.x.set(x, now, animate);
            column.window.configure((width, height).into());
        }
        // Don't leave a view past the last column.
        let end = (self.column_x(self.columns.len()) - self.size.w as f64).max(0.0);
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

    /// Where each window is at `now`, as `seat` sees it on screen.
    pub fn positions(&self, seat: usize, now: Instant) -> impl Iterator<Item = (&W, Point<i32, Logical>)> {
        let view = self.view(seat, now);
        self.columns.iter().map(move |c| {
            let x = (c.x.value(now) - view).round() as i32;
            (&c.window, Point::from((x, GAP)))
        })
    }

    /// Where each window is headed along the workspace, before any view.
    pub fn targets(&self) -> impl Iterator<Item = (&W, Rectangle<i32, Logical>)> {
        let height = (self.size.h - 2 * GAP).max(1);
        self.columns.iter().enumerate().map(move |(i, c)| {
            let x = c.x.target().round() as i32;
            (&c.window, Rectangle::new((x, GAP).into(), (self.column_width(i), height).into()))
        })
    }

    /// Whether the columns, or `seat`'s view, are still moving.
    pub fn is_animating(&self, seat: usize, now: Instant) -> bool {
        self.views.get(seat).is_some_and(|v| !v.is_done(now)) || self.columns.iter().any(|c| !c.x.is_done(now))
    }

    fn column_width(&self, index: usize) -> i32 {
        let fraction = self.columns.get(index).map_or(DEFAULT_WIDTH, |c| c.width);
        ((self.size.w as f64 * fraction) as i32 - GAP - GAP / 2).max(1)
    }

    fn column_x(&self, index: usize) -> f64 {
        (0..index).fold(GAP as f64, |x, i| x + (self.column_width(i) + GAP) as f64)
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
