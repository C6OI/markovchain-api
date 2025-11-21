use crate::settings::DatabaseSettings;
use anyhow::Result;
use deadpool_postgres::Pool;
use tokio_postgres::NoTls;

pub async fn create_pool(settings: &DatabaseSettings) -> Result<Pool> {
    let pool = settings
        .pool
        .create_pool(Some(deadpool_postgres::Runtime::Tokio1), NoTls)?;
    Ok(pool)
}
