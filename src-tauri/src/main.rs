// Use the GUI subsystem for release builds so Windows does not open a console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() { region_watch_lib::run(); }
