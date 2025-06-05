use sqlx::{Pool, Postgres};
use std::io::{Error, ErrorKind};
use uuid::Uuid;

use crate::models::BestlightCase;

impl BestlightCase {
	pub async fn get_cases(db: &Pool<Postgres>, offset: i64) -> Result<Vec<Self>, Error> {
		let cases_query_result = sqlx::query_as!(
			BestlightCase,
			"SELECT * FROM bestlight_cases ORDER by case_id LIMIT 1 OFFSET $1",
			&offset
		)
		.fetch_all(db)
		.await;

		if cases_query_result.is_err() {
			println!("Что-то пошло не так во время запроса get_cases");
		}

		Ok(cases_query_result.unwrap_or(Vec::new()))
	}
}
