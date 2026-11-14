//! Backends and drawing: nested in a window (`winit`) or on a VT (`udev`),
//! client dmabufs, and the scene they draw (`shot`).

pub mod dmabuf;
pub mod shot;
pub mod udev;
pub mod winit;
