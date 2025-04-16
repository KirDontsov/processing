use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, sqlx::FromRow, Serialize, Clone)]
pub struct BestlightCase {
	pub case_id: Uuid,
	pub name: Option<String>,
	pub complete_ts: Option<String>,
	pub transaction_id: Option<String>,
	pub description: Option<String>,
	pub prices: Option<String>,
	pub expenses: Option<String>,
	pub photo: Option<String>,
	pub url: Option<String>,
	pub oai_description: Option<String>,
	pub retail_price: Option<String>,
	#[serde(rename = "createdTs")]
	pub created_ts: Option<DateTime<Utc>>,
	pub vin: Option<String>,
}
