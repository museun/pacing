#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

mod progress;
mod view;

use pacing_core::*;

mod main_window;
pub use main_window::MainWindow;
