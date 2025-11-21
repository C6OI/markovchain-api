use anyhow::Result;
use config::{Config, ConfigError};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseSettings {
    pub pool: deadpool_postgres::Config,
}

impl Settings {
    pub fn parse() -> Result<Self, ConfigError> {
        let settings = Config::builder()
            .add_source(config::File::with_name("config/settings"))
            .build()?;

        settings.try_deserialize()
    }
}
