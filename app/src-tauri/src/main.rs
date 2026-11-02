// Release builds use the GUI subsystem on Windows; CLI subcommands attach to the
// launching console before printing (SPEC 5.2).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tailcopy_app::{Command, attach_console, forward};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match Command::parse(&args) {
        Ok(c) => c,
        Err(message) => {
            attach_console();
            println!("{message}");
            return;
        }
    };
    let autostart = args.iter().any(|a| a == "--autostart");

    // Another instance running? Hand it the command and leave.
    if let Ok(Some(reply)) = tauri::async_runtime::block_on(forward(command)) {
        if !reply.is_empty() {
            attach_console();
            match serde_json::from_str::<serde_json::Value>(&reply) {
                Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap_or(reply)),
                Err(_) => println!("{reply}"),
            }
        }
        return;
    }
    tailcopy_app::run(command, autostart);
}
