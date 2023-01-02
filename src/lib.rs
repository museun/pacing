#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

mod config;
mod format;
mod lingo;
mod mechanics;
mod progress;
mod rand;
mod view;

mod main_window;
pub use main_window::MainWindow;
