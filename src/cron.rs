// Function to drop expired partitions
pub async fn cleanup_expired_partitions(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    let expired_month = chrono::Utc::now()
        .date_naive()
        .checked_sub_months(chrono::Months::new(1))
        .unwrap();

    let partition_name = format!(
        "urls_y{}m{}",
        expired_month.format("%Y"),
        expired_month.format("%m")
    );

    sqlx::query(&format!("DROP TABLE IF EXISTS {}", partition_name))
        .execute(pool)
        .await?;

    Ok(())
}
