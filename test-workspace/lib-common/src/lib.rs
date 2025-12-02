// Base library with no dependencies
pub fn get_app_name() -> &'static str {
    "TestApp"
}

pub fn get_version() -> &'static str {
    "1.0.0"
}

pub struct Config {
    pub debug: bool,
    pub verbose: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            debug: false,
            verbose: false,
        }
    }
}

