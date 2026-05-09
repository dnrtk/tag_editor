use std::sync::Mutex;

use crate::config::Config;

pub struct ServerState {
    pub config: Mutex<Config>,
}

impl ServerState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Mutex::new(config),
        }
    }
}
