use std::cell::RefCell;

use crate::error::Error;
use ic_log::LoggerConfig;

thread_local! {
    static LOGGER_CONFIG: RefCell<Option<LoggerConfig>> = RefCell::new(None);
}

#[derive(Debug, Default)]
/// Handles the runtime logger configuration
pub struct LoggerConfigService {}

impl LoggerConfigService {
    /// Sets a new LoggerConfig
    pub fn init(&self, logger_config: LoggerConfig) {
        LOGGER_CONFIG.with(|config| config.borrow_mut().replace(logger_config));
    }

    /// Changes the logger filter at runtime
    pub fn set_logger_filter(&self, filter: &str) -> Result<(), Error> {
        LOGGER_CONFIG.with(|config| match *config.borrow_mut() {
            Some(ref logger_config) => {
                logger_config.update_filters(filter);
                Ok(())
            }
            None => Err(Error::Internal("LoggerConfig not initialized".to_string())),
        })
    }
}
