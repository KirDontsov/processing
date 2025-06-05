use sqlx::{Pool, Postgres};
use std::io::{Error, ErrorKind};
use uuid::Uuid;

use crate::models::AIDescription;

impl AIDescription {
	pub async fn get_oai_descriptions(
		db: &Pool<Postgres>,
		firm_id: Uuid,
	) -> Result<Vec<Self>, Error> {
		let oai_description_result = sqlx::query_as!(
			AIDescription,
			r#"SELECT * FROM oai_descriptions WHERE firm_id = $1;"#,
			&firm_id
		)
		.fetch_all(db)
		.await;

		if oai_description_result.is_err() {
			println!("Что-то пошло не так во время запроса get_oai_descriptions");
		}

		Ok(oai_description_result.unwrap_or(Vec::new()))
	}

}
