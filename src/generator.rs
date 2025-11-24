use anyhow::bail;
use deadpool_postgres::Pool;
use rand::Rng;
use rand_distr::Distribution;
use rand_distr::LogNormal;

pub async fn generate_text(
    pool: &Pool,
    text_length: Option<usize>,
    start: Option<String>,
) -> anyhow::Result<String> {
    let client = pool.get().await?;
    let statement = client
        .prepare_cached(
            /* language=postgresql */
            r#"
            SELECT "to", count
            FROM chain_entries
            WHERE "from" = $1
            "#,
        )
        .await?;

    let text_length = match text_length {
        Some(length) => length,
        None => calc_text_length(pool).await?,
    };

    let mut text = start.map_or_else(
        || String::with_capacity(text_length),
        |v| v.trim().to_string(),
    );

    let mut last_word = text.clone().split(' ').next_back().unwrap().to_string();

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

        let word = get_random_weighted_word(&available_entries)?;

        if !text.is_empty() {
            text.push(' ');
        }

        text.push_str(word);
        last_word.clone_from(word);
    }

    Ok(text)
}

fn get_random_weighted_word(available_entries: &[(String, i32)]) -> anyhow::Result<&String> {
    let total = available_entries.iter().fold(0, |acc, entry| acc + entry.1);
    let mut r = rand::rng().random_range(0..=total);

    for entry in available_entries {
        r -= entry.1;

        if r <= 0 {
            return Ok(&entry.0);
        }
    }

    bail!("Failed to get a random entry")
}

async fn calc_text_length(pool: &Pool) -> anyhow::Result<usize> {
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

    let sample = log_normal.sample(&mut rand::rng()).round();

    Ok(sample as usize)
}
