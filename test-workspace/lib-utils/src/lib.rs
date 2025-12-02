use lib_common::{get_app_name, Config};

pub fn format_message(msg: &str) -> String {
    format!("[{}] {}", get_app_name(), msg)
}

pub fn log_with_config(msg: &str, config: &Config) {
    if config.verbose {
        println!("{}", format_message(msg));
    }
}

pub fn capitalize(s: &str) -> String {
    s.chars()
        .enumerate()
        .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
        .collect()
}

