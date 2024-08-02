use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, sqlx::FromRow, Serialize, Clone, Default)]
pub struct Counter {
	pub counter_id: Uuid,
	pub value: Option<String>,
	pub name: Option<String>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, sqlx::FromRow, Serialize, Clone, Default)]
pub struct SaveCounter {
	pub counter_id: Uuid,
	pub value: String,
}
