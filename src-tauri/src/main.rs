#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("--cli") {
        std::process::exit(funo_studio_lib::cli::run());
    }
    if args.iter().any(|value| value == "--install-path") {
        std::process::exit(if funo_studio_lib::path_setup::install().is_ok() { 0 } else { 1 });
    }
    if args.iter().any(|value| value == "--uninstall-path") {
        std::process::exit(if funo_studio_lib::path_setup::uninstall().is_ok() { 0 } else { 1 });
    }
    if let Some(value) = args.iter().find_map(|value| value.strip_prefix("--installer-beginner=")) {
        let beginner = matches!(value, "1" | "true" | "yes");
        std::process::exit(if funo_studio_lib::settings::set_installer_beginner(beginner).is_ok() { 0 } else { 1 });
    }
    funo_studio_lib::run();
}
