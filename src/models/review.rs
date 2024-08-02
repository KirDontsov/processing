use chrono::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, sqlx::FromRow, Serialize, Clone)]
pub struct Review {
	pub review_id: uuid::Uuid,
	pub firm_id: uuid::Uuid,
	pub two_gis_firm_id: Option<String>,
	pub author: Option<String>,
	pub date: Option<String>,
	pub rating: Option<String>,
	pub text: Option<String>,
	pub parsed: Option<bool>,
	#[serde(rename = "createdTs")]
	pub created_ts: Option<DateTime<Utc>>,
}
