use anyhow::{bail, Result};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use tokio::fs;
use tokio_postgres::Client;

#[allow(unused)]
pub struct Migrations {
    table_name: String,
    up: HashMap<String, String>,
    down: HashMap<String, String>,
}

#[allow(unused)]
impl Migrations {
    pub fn new(table_name: String, migrations_path: &Path) -> Result<Self> {
        let mut up = HashMap::new();
        let mut down = HashMap::new();

        for entry in migrations_path
            .read_dir()
            .expect("Failed to read migrations dir")
            .flatten()
        {
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let name = entry.file_name().clone().into_string().unwrap();
            let path = entry.path();

            let up_path = path.join("up.sql");
            let down_path = path.join("down.sql");

            if !up_path.exists() {
                bail!("'{}' not found", up_path.to_str().unwrap())
            }

            if !down_path.exists() {
                bail!("'{}' not found", down_path.to_str().unwrap())
            }

            up.insert(
                name.clone(),
                up_path.into_os_string().into_string().unwrap(),
            );

            down.insert(name, down_path.into_os_string().into_string().unwrap());
        }

        Ok(Self {
            table_name,
            up,
            down,
        })
    }

    async fn execute_script(&self, client: &Client, content: &str) -> Result<()> {
        client.batch_execute(content).await?;
        Ok(())
    }

    async fn insert_migration(&self, client: &Client, name: &String) -> Result<()> {
        let query = format!("INSERT INTO {} (name) VALUES ($1)", self.table_name);
        let statement = client.prepare(&query).await?;
        client.execute(&statement, &[&name]).await?;
        Ok(())
    }

    async fn delete_migration(&self, client: &Client, name: &String) -> Result<()> {
        let query = format!("DELETE FROM {} WHERE name = $1", self.table_name);
        let statement = client.prepare(&query).await?;
        client.execute(&statement, &[&name]).await?;
        Ok(())
    }

    async fn create_table(&self, client: &Client) -> Result<()> {
        tracing::debug!("Ensuring migration table {}", self.table_name);
        let query = format!(
            r"CREATE TABLE IF NOT EXISTS {} ( name TEXT NOT NULL PRIMARY KEY, executed_at TIMESTAMP NOT NULL DEFAULT NOW() )",
            self.table_name
        );
        self.execute_script(client, &query).await?;
        Ok(())
    }

    async fn exists(&self, client: &Client, name: &String) -> Result<bool> {
        tracing::trace!("Check if migration {} exists", name);
        let query = format!("SELECT COUNT(*) FROM {} WHERE name = $1", self.table_name);
        let statement = client.prepare(&query).await?;
        let row = client.query_one(&statement, &[&name]).await?;
        let count: i64 = row.get(0);

        Ok(count > 0)
    }

    /// Migrate all scripts up
    pub async fn up(&self, client: &Client) -> Result<()> {
        tracing::info!("Migrating up to table '{}'", self.table_name);

        self.create_table(client).await?;

        let mut names_sorted: Vec<_> = self.up.keys().collect();
        names_sorted.sort_by_key(|key| u64::from_str(key.split('_').next().unwrap()).unwrap());

        for name in names_sorted {
            if !self.exists(client, name).await? {
                tracing::debug!("Applying migration {}", name);

                let path = &self.up[name];
                let content = fs::read_to_string(path).await?;
                self.execute_script(client, content.as_str()).await?;
                self.insert_migration(client, name).await?;
            }
        }
        Ok(())
    }

    /// Migrate all scripts down
    pub async fn down(&self, client: &Client) -> Result<()> {
        tracing::info!("Migrating down to table '{}'", self.table_name);

        self.create_table(client).await?;

        let mut names_sorted: Vec<_> = self.down.keys().collect();
        names_sorted.sort_by_key(|key| u64::from_str(key.split('_').next().unwrap()).unwrap());

        for name in names_sorted {
            if self.exists(client, name).await? {
                tracing::debug!("Deleting migration {}", name);

                let path = &self.down[name];
                let content = fs::read_to_string(path).await?;
                self.execute_script(client, content.as_str()).await?;
                self.delete_migration(client, name).await?;
            }
        }
        Ok(())
    }
}
