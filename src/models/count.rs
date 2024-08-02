use serde::Deserialize;
use sqlx::FromRow;

#[derive(Deserialize, Debug, FromRow)]
pub struct Count {
	pub count: Option<i64>,
}
