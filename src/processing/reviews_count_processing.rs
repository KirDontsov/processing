use crate::models::{Count, Firm};
use sqlx::{Pool, Postgres};

pub async fn reviews_count_processing(
	pool: Pool<Postgres>,
) -> Result<(), Box<dyn std::error::Error>> {
	println!("start");
	let table_name = String::from("firms");

	let firms_count =
		Count::count_firms_with_empty_field(&pool, table_name.clone(), "reviews_count".to_string())
			.await
			.unwrap_or(0);

	for j in 0..=firms_count {
		println!("№ {}", &j);
		let firm = Firm::get_firm_with_empty_field(
			&pool,
			table_name.clone(),
			"reviews_count".to_string(),
			j,
		)
		.await
		.unwrap();

		if firm.reviews_count.clone().is_some() {
			continue;
		}

		let count_query_result = sqlx::query_as!(
			Count,
			"SELECT count(*) AS count FROM reviews WHERE firm_id = $1",
			firm.firm_id
		)
		.fetch_one(&pool)
		.await;

		let reviews_count = match count_query_result {
			Ok(x) => x,
			Err(_) => Count { count: Some(0_i64) },
		};

		let _ = sqlx::query_as::<_, Firm>(
			r#"UPDATE firms SET reviews_count = $1 WHERE firm_id = $2 RETURNING *"#,
		)
		.bind(reviews_count.count.unwrap().to_string())
		.bind(firm.firm_id)
		.fetch_one(&pool)
		.await;

		dbg!(&reviews_count.count);
	}

	Ok(())
}
