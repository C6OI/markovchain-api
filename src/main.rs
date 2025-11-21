use rand_distr::Distribution;
mod database;
mod migrations;
mod settings;

use crate::database::create_pool;
use crate::migrations::Migrations;
use crate::settings::Settings;
use anyhow::{bail, Context, Result};
use deadpool_postgres::Pool;
use rand::rngs::ThreadRng;
use rand::Rng;
use rand_distr::LogNormal;
use std::io;
use std::io::{stdout, Write};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    let settings = Settings::parse().unwrap_or_else(|err| panic!("Failed to load settings: {err}"));

    let pool = create_pool(&settings.database).await?;
    let client = pool.get().await?;

    Migrations::new("version_info".into(), Path::new("migrations"))?
        .up(&client)
        .await?;

    let mut rng = rand::rng();

    loop {
        let mut input = String::new();

        print!("Input: ");
        stdout().flush()?;
        io::stdin().read_line(&mut input)?;
        println!();

        process_input(&pool, input).await?;

        let text = generate_text(&pool, &mut rng).await?;
        println!("{}", text);
    }
}

async fn process_input(pool: &Pool, input: String) -> Result<()> {
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

async fn increment_or_create_entry(pool: &Pool, from: &str, to: &str) -> Result<()> {
    let from = from.trim();
    let to = to.trim();

    if to.is_empty() {
        return Ok(());
    }

    let client = pool.get().await?;

    let statement = client
        .prepare_cached(
            /* language=postgresql */
            r##"
        INSERT INTO chain_entries ("from", "to", count)
        VALUES ($1, $2, 1)
        ON CONFLICT ("from", "to") DO UPDATE
        SET count = chain_entries.count + 1
        WHERE chain_entries.from = $1 AND chain_entries.to = $2"##,
        )
        .await?;

    client.execute(&statement, &[&from, &to]).await?;
    Ok(())
}

async fn generate_text(pool: &Pool, rng: &mut ThreadRng) -> Result<String> {
    let text_length = calc_text_length(pool, rng).await?;

    let mut text = String::with_capacity(text_length);
    let mut last_word = String::new();

    let client = pool.get().await?;
    let statement = client
        .prepare_cached(
            /* language=postgresql */
            r##"
            SELECT "to", count
            FROM chain_entries
            WHERE "from" = $1
            "##,
        )
        .await?;

    while text.len() < text_length {
        let available_entries: Vec<(String, i32)> = client
            .query(&statement, &[&last_word])
            .await?
            .iter()
            .map(|row| (row.get(0), row.get(1)))
            .collect();

        if available_entries.is_empty() {
            break;
        }

        let word = get_random_weighted_word(&available_entries, rng)?;

        text.push_str(word);
        text.push(' ');
        last_word = word.clone();
    }

    Ok(text)
}

fn get_random_weighted_word<'a>(
    available_entries: &'a [(String, i32)],
    rng: &mut ThreadRng,
) -> Result<&'a String> {
    let total = available_entries.iter().fold(0, |acc, entry| acc + entry.1);
    let mut r = rng.random_range(0..=total);

    for entry in available_entries {
        r -= entry.1;

        if r <= 0 {
            return Ok(&entry.0);
        }
    }

    bail!("Failed to get a random entry")
}

async fn calc_text_length(pool: &Pool, rng: &mut ThreadRng) -> Result<usize> {
    let client = pool.get().await?;

    let statement = client
        .prepare_cached(
            /* language=postgresql */
            r"
        WITH lengths as (
            SELECT length(content) as len
            FROM texts
        )
        SELECT AVG(len)::DOUBLE PRECISION, STDDEV_POP(len)::DOUBLE PRECISION
        FROM lengths;
        ",
        )
        .await?;

    let row = client.query_one(&statement, &[]).await?;

    let avg: f64 = row.get(0);
    let std_dev: f64 = row.get(1);

    // coefficient of variation
    let cv = std_dev / avg;
    let log_normal: LogNormal<f64> = LogNormal::from_mean_cv(avg, cv)?;

    let sample = log_normal.sample(rng).round();

    Ok(sample as usize)
}
