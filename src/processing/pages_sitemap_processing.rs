use sitemap::structs::UrlEntry;
use sitemap::writer::SiteMapWriter;
use sqlx::{Pool, Postgres};
use std::env;
use std::io::stdout;

use crate::models::{Category, City, Count, Firm, Page};

pub async fn pages_sitemap_processing(
	pool: Pool<Postgres>,
) -> Result<(), Box<dyn std::error::Error>> {
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
	let city_name = env::var("CRAWLER_CITY_NAME").expect("CRAWLER_CITY_NAME not set");
	let category_name = env::var("CRAWLER_CATEGOTY_NAME").expect("CRAWLER_CATEGOTY_NAME not set");
	let rubric_id = env::var("CRAWLER_RUBRIC_ID").expect("CRAWLER_RUBRIC_ID not set");
	let url = env::var("CRAWLER_FIRM_URL").expect("CRAWLER_FIRM_URL not set");
	let domain = "https://xn--90ab9accji9e.xn--p1ai";

	let city = sqlx::query_as!(
		City,
		"SELECT * FROM cities WHERE city_id = $1;",
		city_id.clone()
	)
	.fetch_one(&pool)
	.await
	.unwrap();

	let category = sqlx::query_as!(
		Category,
		"SELECT * FROM categories WHERE category_id = $1;",
		category_id.clone()
	)
	.fetch_one(&pool)
	.await
	.unwrap();

	let mut output = stdout();
	let sitemap_writer = SiteMapWriter::new(&mut output);
	let mut urlwriter = sitemap_writer
		.start_urlset()
		.expect("Unable to write urlset");

	let firm = Firm::get_firm_by_url(&pool, &url)
		.await
		.expect("there is no firm");

	let pages_count = Count::count_pages_by_firm(&pool, firm.firm_id.to_string())
		.await
		.unwrap_or(0);

	for j in 0..=pages_count {
		let pages_res = Page::get_pages_by_firm(&pool, &firm.firm_id.clone(), 1, j)
			.await
			.expect("there is no page");

		let page = &pages_res[0];

		if firm.url.clone().is_none() {
			continue;
		}

		let url = format!(
			"{}/{}/{}/{}/cases/{}",
			&domain.clone(),
			&city.abbreviation.clone().unwrap(),
			&category.abbreviation.clone().unwrap(),
			&firm.url.clone().unwrap(),
			&page.url.clone().unwrap()
		);

		urlwriter
			.url(UrlEntry::builder().loc(&url))
			.expect("Unable to write url");
	}

	urlwriter.end().expect("Unable to write close tags");

	Ok(())
}
