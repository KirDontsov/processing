use chrono::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, sqlx::FromRow, Serialize, Clone)]
pub struct AIReview {
	pub oai_review_id: uuid::Uuid,
	pub firm_id: uuid::Uuid,
	pub text: Option<String>,
	#[serde(rename = "createdTs")]
	pub created_ts: Option<DateTime<Utc>>,
	#[serde(rename = "updatedTs")]
	pub updated_ts: Option<DateTime<Utc>>,
}
