use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::postgres::types::TsVector;
use sqlx::{FromRow, Type};
use uuid::Uuid;

#[allow(non_snake_case)]
#[derive(Debug, sqlx::FromRow, sqlx::Type, Default)]
pub struct Firm {
	pub firm_id: Uuid,
	pub category_id: Uuid,
	pub type_id: Uuid,
	pub city_id: Uuid,
	pub two_gis_firm_id: Option<String>,
	pub name: Option<String>,
	pub description: Option<String>,
	pub address: Option<String>,
	pub floor: Option<String>,
	pub site: Option<String>,
	pub default_email: Option<String>,
	pub default_phone: Option<String>,
	pub url: Option<String>,
	pub rating: Option<String>,
	pub reviews_count: Option<String>,
	pub coords: Option<String>,
	pub title: Option<String>,
	pub ts: Option<TsVector>,
	pub created_ts: Option<DateTime<Utc>>,
	pub updated_ts: Option<DateTime<Utc>>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, FromRow, Serialize, Clone)]
pub struct UpdateFirmDesc {
	pub firm_id: Uuid,
	pub description: String,
}
