use anyhow::Context;
use deadpool_postgres::Pool;

pub async fn process_input(pool: &Pool, input: String) -> anyhow::Result<()> {
    let input = input.trim();

    if input.is_empty() {
        return Ok(());
    }

    {
        let client = pool.get().await?;

        let statement = client
            .prepare_cached(
                /* language=postgresql */ "INSERT INTO texts (content) VALUES ($1)",
            )
            .await
            .context("Failed to prepare insert texts statement")?;

        client
            .query(&statement, &[&input])
            .await
            .context("Failed to query insert texts statement")?;
    }

    let mut tasks = Vec::new();

    let parts: Vec<&str> = input.split(' ').collect();
    tasks.push(increment_or_create_entry(pool, "", parts[0]));

    // len() - 1 because we don't want to handle the last part separately
    for i in 1..parts.len() - 1 {
        let current = parts[i];
        let previous = parts[i - 1];

        tasks.push(increment_or_create_entry(pool, previous, current));
    }

    for res in futures::future::join_all(tasks).await {
        res?; // unwrapping every result
    }

    Ok(())
}

async fn increment_or_create_entry(pool: &Pool, from: &str, to: &str) -> anyhow::Result<()> {
    let from = from.trim();
    let to = to.trim();

    if to.is_empty() {
        return Ok(());
    }

    let client = pool.get().await?;

    let statement = client
        .prepare_cached(
            /* language=postgresql */
            r#"
        INSERT INTO chain_entries ("from", "to", count)
        VALUES ($1, $2, 1)
        ON CONFLICT ("from", "to") DO UPDATE
        SET count = chain_entries.count + 1
        WHERE chain_entries.from = $1 AND chain_entries.to = $2"#,
        )
        .await?;

    client.execute(&statement, &[&from, &to]).await?;
    Ok(())
}
