// Use the GUI subsystem on Windows release builds so no console window appears.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    ferry_app::run();
}
