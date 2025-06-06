use crate::{
	models::{Count, Firm},
	utils::Translit,
};
use sqlx::{Pool, Postgres};
use std::env;
use urlencoding::encode;

pub async fn urls_processing(pool: Pool<Postgres>) -> Result<(), Box<dyn std::error::Error>> {
	println!("start");
	let table = String::from("firms");
	let city_id = uuid::Uuid::parse_str(
		env::var("CRAWLER_CITY_ID")
			.expect("CRAWLER_CITY_ID not set")
			.as_str(),
	)
	.unwrap();
	let category_id = uuid::Uuid::parse_str(
		env::var("CRAWLER_CATEGORY_ID")
			.expect("CRAWLER_CATEGORY_ID not set")
			.as_str(),
	)
	.unwrap();

	let firms_count = Count::count_firms_with_empty_field(&pool, table.clone(), "url".to_string())
		.await
		.unwrap_or(0);

	for j in 0..=firms_count {
		println!("№ {}", &j);
		let firm = Firm::get_firm_with_empty_field(&pool, table.clone(), "url".to_string(), j)
			.await
			.unwrap();

		// let firm = Firm::get_firm_by_city_category(&pool, table.clone(), city_id, category_id, j)
		// 	.await
		// 	.unwrap();

		if firm.url.clone().is_some() {
			continue;
		}

		let translit_name = Translit::convert(firm.name.clone());
		let firm_address = firm.address.clone().unwrap_or("".to_string());
		let firm_street = firm_address.split(",").collect::<Vec<&str>>()[0].to_string();
		let firm_house = firm_address.split(",").collect::<Vec<&str>>()[1].to_string();
		let translit_address = if firm_address != "" {
			Translit::convert(Some(format!("{}-{}", firm_street, firm_house)))
		} else {
			firm.firm_id.clone().to_string()
		};

		let mut firm_url = String::new();

		let firms_double_urls = sqlx::query_as::<_, Firm>(r#"SELECT * FROM firms WHERE url = $1"#)
			.bind(&firm.url.clone().unwrap_or("".to_string()))
			.fetch_all(&pool)
			.await?;

		if firms_double_urls.len() > 0 {
			firm_url = format!(
				"{}-{}-{}",
				&translit_name,
				&translit_address,
				&firm.firm_id.clone()
			);
		} else {
			firm_url = format!("{}-{}", &translit_name, &translit_address);
		}

		let _ = sqlx::query_as::<_, Firm>(
			r#"UPDATE firms SET url = $1 WHERE firm_id = $2 RETURNING *"#,
		)
		.bind(encode(
			firm_url
				.replace(" ", "-")
				.replace(",", "-")
				.replace(".", "-")
				.replace("`", "")
				.replace("/", "-")
				.replace("(", "-")
				.replace(")", "-")
				.replace("&amp;", "&")
				.replace("--", "-")
				.as_str(),
		))
		.bind(firm.firm_id)
		.fetch_one(&pool)
		.await;

		dbg!(&firm.firm_id.clone());
		dbg!(&firm_url);
	}

	Ok(())
}
