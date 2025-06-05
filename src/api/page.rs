use sqlx::{Pool, Postgres};
use std::io::{Error, ErrorKind};
use uuid::Uuid;

use crate::models::Page;

impl Page {
	pub async fn get_pages_by_firm(
		db: &Pool<Postgres>,
		id: &Uuid,
		limit: i64,
		offset: i64,
	) -> Result<Vec<Self>, Error> {
		let pages_query_result = sqlx::query_as::<_, Page>(
			"SELECT * FROM pages WHERE firm_id = $1 ORDER BY created_ts LIMIT $2 OFFSET $3",
		)
		.bind(id)
		.bind(&limit)
		.bind(&offset)
		.fetch_all(db)
		.await;

		let message = "Что-то пошло не так во время запроса get_pages_by_firm";

		Ok(pages_query_result.expect(&message))
	}
}
