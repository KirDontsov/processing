use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Pool, Postgres};
use std::env;
use std::error::Error;
use tokio::time::{sleep, Duration};
use uuid::Uuid;
use urlencoding::encode;

use crate::{
	models::{Count, Counter, Firm, Review, SaveCounter, BestlightCase, Page, PageBlock, PageBlockSection},
	utils::Translit
};

#[derive(Debug, Deserialize, Serialize)]
struct AccessToken {
	access_token: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ApiResponse {
	message: Message,
}

#[derive(Debug, Deserialize, Serialize)]
struct Choice {
	message: Message,
}

#[derive(Debug, Deserialize, Serialize)]
struct Message {
	content: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Scope {
	scope: String,
}

pub async fn oai_pages_processing(pool: Pool<Postgres>) -> Result<(), Box<dyn Error>> {
	let url = env::var("OPENAI_API_BASE").expect("OPEN_AI_API_KEY not set");
	let open_ai_token = env::var("OPENAI_API_KEY").expect("OPEN_AI_API_KEY not set");

	let counter_id: String = String::from("23cae330-9a3d-4655-8b88-5cfaaad914a3");
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
	let table = String::from("firms");
	let model_name = "deepseek-v2:16b";

	let split_target = String::from("\n");

	let headers: HeaderMap<HeaderValue> = header::HeaderMap::from_iter(vec![
		(header::ACCEPT, "application/json".parse().unwrap()),
		(header::CONTENT_TYPE, "application/json".parse().unwrap()),
	]);

	// получаем из базы кол-во фирм
	let firms_count_res =
		sqlx::query_as!(
			Count,
			"SELECT count(*) AS count FROM firms WHERE firm_id = $1",
			Uuid::parse_str(&"130f13e0-1853-4dd4-8b5b-03712fb20057").unwrap()
		)
		.fetch_one(&pool)
		.await?;

	let firms_count = firms_count_res.count.expect("firms count not exist");

	// получаем из базы начало счетчика
	// let start = Counter::get_counter(&pool, &counter_id)
	// 	.await?
	// 	.value
	// 	.unwrap_or(String::from("0"))
	// 	.parse::<i64>()
	// 	.unwrap_or(0);

	for j in 0..=firms_count {
		println!("Firm: {:?}", j + 1);

		let firm = Firm::get_firm_by_url(&pool, &"luchshii-svet-tihaya-6-lit-m".to_string()).await?;
		let firm_name = firm.name.expect("firm name not exist");

		dbg!(&firm_name);

		if firm_name == "" {
			continue;
		}

		let count_query_result = sqlx::query_as!(
			Count,
			"SELECT count(*) AS count FROM bestlight_cases",
		)
		.fetch_one(&pool)
		.await;

		let cases_count = match count_query_result {
			Ok(x) => x,
			Err(_) => Count { count: Some(0_i64) },
		};

		if cases_count.count.unwrap_or(0) == 0 {
			continue;
		}

		dbg!(&cases_count);

		for i in 0..=cases_count.count.unwrap_or(0) {
			println!("{}", &i);

			let cases_by_firm = BestlightCase::get_cases(&pool, i)
				.await
				.unwrap_or(Vec::new());

			if cases_by_firm.len() == 0 {
				continue;
			}

			let cur_case = cases_by_firm.get(0).unwrap().to_owned();

			let mut firm_url = String::new();
			let case_id = cur_case.case_id.clone();
			let case_name = cur_case.name.clone();
			let translit_name = Translit::convert(case_name.clone());

			dbg!(&case_name.clone().unwrap_or("".to_string()));

			let pages_double_urls = sqlx::query_as::<_, Page>(r#"SELECT * FROM pages WHERE oai_value = $1"#)
				.bind(&case_name.clone().unwrap_or("".to_string()))
				.fetch_all(&pool)
				.await?;

			if pages_double_urls.len() != 0 {
				continue;
			}

			let cases_double_urls = sqlx::query_as::<_, BestlightCase>(r#"SELECT * FROM bestlight_cases WHERE name = $1"#)
				.bind(&case_name.clone().unwrap_or("".to_string()))
				.fetch_all(&pool)
				.await?;

			if cases_double_urls.len() > 1 {
				firm_url = format!(
					"{}-{}",
					&translit_name,
					&case_id,
				);
			} else {
				firm_url = format!("{}", &translit_name);
			}

			let prepared_firm_url = firm_url
				.replace(" ", "-")
				.replace(",", "-")
				.replace(".", "-")
				.replace("`", "")
				.replace("/", "-")
				.replace("&amp;", "&")
				.replace("--", "-");

			// запись в бд
			let created_page = sqlx::query_as!(
				Page,
				r#"INSERT INTO pages (url, firm_id, oai_value, page_photo) VALUES ($1, $2, $3, $4) RETURNING *"#,
				&encode(&prepared_firm_url.as_str()),
				firm.firm_id.clone(),
				case_name.clone().unwrap_or("".to_string()),
				cur_case.photo.clone()
			)
			.fetch_one(&pool)
			.await?;


			let cur_page_id = created_page.clone().page_id;
			dbg!(&cur_page_id);

			let block_title = format!("Кейс ремонт фар {}", case_name.clone().unwrap_or("".to_string()));
			let case_description = cur_case.oai_description.unwrap_or("".to_string());

			let page_block = sqlx::query_as!(
				PageBlock,
				r#"INSERT INTO pages_blocks (page_id, page_block_order, page_block_title, page_block_type) VALUES ($1, $2, $3, $4) RETURNING *"#,
				cur_page_id,
				"0".to_string(),
				block_title,
				1,
			)
			.fetch_one(&pool)
			.await?;

			// === KEY POINTS ===
			let key_points_preamble = format!(
				"
				The Text:
				{}
				",
				&case_description
			);

			let key_points_body = json!({
			  "model": model_name,
				"stream": false,
			  "messages": [
					{
						"role": "system",
						"content": "
						1. Act as a professional text analizer.
						2. Context: I will provide you with the Text.
						3. Your task: Analyze the text and highlight three key points without prices.
						4. Format: Write your answer only in the Russian language, do not use Chinese words/hieroglyphs or English words/letters or other languages, only Russian language. Write in plain text. Give the answer in listicle form. Each item from a new line. Keep the meaning and write from the same person as in the text. Write in the first person and preserve the speaker's gender. Try to write as a man. Don't write what you think and don't use any system phrases in your answer. Do not use phrases like: *Ответ:*
						5. Tone of Voice: Be empathetic, concise, intelligent, driven, and wise. Think step by step."
					},
					{
						"role": "user",
						"content": &key_points_preamble.replace("\t", "").replace("\n", "")
					}
				]
			});

			// request
			let key_points_response = reqwest::Client::builder()
				.danger_accept_invalid_certs(true)
				.build()
				.unwrap()
				.post(url.clone())
				.headers(headers.clone())
				.json(&key_points_body)
				.send()
				.await?;

			let empty_message = Message {
				content: "".to_string(),
			};

			let key_points_res: ApiResponse = match key_points_response.json().await {
				Ok(result) => result,
				Err(e) => {
					println!("Network error: {:?}", e);
					ApiResponse {
						message: empty_message,
					}
				}
			};

			let key_points_oai_res = &key_points_res.message.content;

			if key_points_oai_res == "" {
				continue;
			}

			// response
			println!("============");
			println!("{:?}", &key_points_oai_res);

			let block_title = format!("Кейс ремонт фар {}", case_name.clone().unwrap_or("".to_string()));

			let key_points_block = sqlx::query_as!(
				PageBlock,
				r#"INSERT INTO pages_blocks (page_id, page_block_order, page_block_title, page_block_type) VALUES ($1, $2, $3, $4) RETURNING *"#,
				cur_page_id,
				"1".to_string(),
				"Ключевые моменты:".to_string(),
				1,
			)
			.fetch_one(&pool)
			.await?;

			let key_points_block_id = key_points_block.clone().page_block_id;
			dbg!(&key_points_block_id);

			let mut key_points_oai_array = key_points_oai_res
				.split(&split_target)
				.filter(|&x| *x != *"   ")
				.collect::<Vec<&str>>();

			for (index, key) in key_points_oai_array.iter().enumerate() {
				if *key != "".to_string() && *key != " ".to_string() {
					let _ = sqlx::query_as!(
						PageBlockSection,
						r#"INSERT INTO pages_blocks_sections (page_block_id, page_block_section_order, text) VALUES ($1, $2, $3) RETURNING *;"#,
        	key_points_block_id,
         	index.to_string(),
						key.replace("/n", ""),
					)
					.fetch_one(&pool)
					.await?;
				}
			}

			// === WORK STAGES ===
			let work_stages_block = sqlx::query_as!(
				PageBlock,
				r#"INSERT INTO pages_blocks (page_id, page_block_order, page_block_title, page_block_type) VALUES ($1, $2, $3, $4) RETURNING *"#,
				cur_page_id,
				"2".to_string(),
				format!("Клиент обратился к нам, испытывая проблемы с передними фарами {}", case_name.clone().unwrap_or("".to_string())),
				1,
			)
			.fetch_one(&pool)
			.await?;

			let work_stages_block_id = work_stages_block.clone().page_block_id;
			dbg!(&work_stages_block_id);

			let work_stages_preamble = format!(
				"
				The Text:
				{}
				",
				&case_description
			);

			let work_stages_body = json!({
			  "model": model_name,
				"stream": false,
			  "messages": [
					{
						"role": "system",
						"content": "
						1. Act as a professional text analizer.
						2. Context: I will provide you with the Text.
						3. Your task: Analyze the text and highlight the main stages of the work done without prices.
						4. Format: Write your answer only in the Russian language, do not use Chinese words/hieroglyphs or English words/letters or other languages, only Russian language. Write in plain text. Give the answer in listicle form. Each item from a new line. Keep the meaning and write from the same person as in the text. Write in the first person and preserve the speaker's gender. Try to write as a man. Don't write what you think and don't use any system phrases in your answer. Do not use phrases like: *Ответ:*
						5. Tone of Voice: Be empathetic, concise, intelligent, driven, and wise. Think step by step."
					},
					{
						"role": "user",
						"content": &work_stages_preamble.replace("\t", "").replace("\n", "")
					}
				]
			});

			// request
			let work_stages_response = reqwest::Client::builder()
				.danger_accept_invalid_certs(true)
				.build()
				.unwrap()
				.post(url.clone())
				.headers(headers.clone())
				.json(&work_stages_body)
				.send()
				.await?;

			let empty_message = Message {
				content: "".to_string(),
			};

			let work_stages_res: ApiResponse = match work_stages_response.json().await {
				Ok(result) => result,
				Err(e) => {
					println!("Network error: {:?}", e);
					ApiResponse {
						message: empty_message,
					}
				}
			};

			let work_stages_oai_res = &work_stages_res.message.content;

			// response
			println!("============");
			println!("{:?}", &work_stages_oai_res);

			if work_stages_oai_res == "" {
				continue;
			}

			let mut work_stages_oai_array = work_stages_oai_res
				.split(&split_target)
				.filter(|&x| *x != *"   ")
				.collect::<Vec<&str>>();

			for (index, key) in work_stages_oai_array.iter().enumerate() {
				if *key != "".to_string() && *key != " ".to_string() {
					let _ = sqlx::query_as!(
						PageBlockSection,
						r#"INSERT INTO pages_blocks_sections (page_block_id, page_block_section_order, text) VALUES ($1, $2, $3) RETURNING *;"#,
        	work_stages_block_id,
         	index.to_string(),
						key.replace("/n", ""),
					)
					.fetch_one(&pool)
					.await?;
				}
			}

			let after_work_stages_block = sqlx::query_as!(
				PageBlock,
				r#"INSERT INTO pages_blocks (page_id, page_block_order, page_block_title, page_block_subtitle, page_block_type) VALUES ($1, $2, $3, $4, $5) RETURNING *"#,
				cur_page_id,
				"3".to_string(),
				"Все работы были выполнены качественно и в кратчайшие сроки, что оставило клиента довольным и порекомендовавшим нас своим близким.".to_string(),
				"Если возникает необходимость замены стекол фар, мы используем высококачественные материалы и стекла прямо от производителей или качественные б/у стекла с доноров.".to_string(),
				0,
			)
			.fetch_one(&pool)
			.await?;

			// === PRICES ===
			let prices_block = sqlx::query_as!(
				PageBlock,
				r#"INSERT INTO pages_blocks (page_id, page_block_order, page_block_title, page_block_type) VALUES ($1, $2, $3, $4) RETURNING *"#,
				cur_page_id,
				"4".to_string(),
				format!("Ориентировочные цены на услуги по ремонту и замене стекол фар {}", case_name.clone().unwrap_or("".to_string())),
				1,
			)
			.fetch_one(&pool)
			.await?;

			let prices_block_id = prices_block.clone().page_block_id;
			dbg!(&prices_block_id);

			let prices_preamble = format!(
				"
				The Text:
				{}
				",
				&case_description
			);

			let prices_body = json!({
			  "model": model_name,
				"stream": false,
			  "messages": [
					{
						"role": "system",
						"content": "
						1. Act as a professional text analizer.
						2. Context: I will provide you with the Text.
						3. Your task: Analyze the text and highlight approximate prices for headlight glass repair and replacement services.
						4. Format: Write your answer only in the Russian language, do not use Chinese words/hieroglyphs or English words/letters or other languages, only Russian language. Write in plain text. Give the answer in listicle form. Each item from a new line. Keep the meaning and write from the same person as in the text. Write in the first person and preserve the speaker's gender. Try to write as a man. Don't write what you think and don't use any system phrases in your answer. Do not use phrases like: *Ответ:*
						5. Tone of Voice: Be empathetic, concise, intelligent, driven, and wise. Think step by step."
					},
					{
						"role": "user",
						"content": &prices_preamble.replace("\t", "").replace("\n", "")
					}
				]
			});

			// request
			let prices_response = reqwest::Client::builder()
				.danger_accept_invalid_certs(true)
				.build()
				.unwrap()
				.post(url.clone())
				.headers(headers.clone())
				.json(&prices_body)
				.send()
				.await?;

			let empty_message = Message {
				content: "".to_string(),
			};

			let prices_res: ApiResponse = match prices_response.json().await {
				Ok(result) => result,
				Err(e) => {
					println!("Network error: {:?}", e);
					ApiResponse {
						message: empty_message,
					}
				}
			};

			let prices_oai_res = &prices_res.message.content;

			// response
			println!("============");
			println!("{:?}", &prices_oai_res);

			if prices_oai_res == "" {
				continue;
			}

			let mut prices_oai_array = prices_oai_res
				.split(&split_target)
				.filter(|&x| *x != *"   ")
				.collect::<Vec<&str>>();

			for (index, key) in prices_oai_array.iter().enumerate() {
				if *key != "".to_string() && *key != " ".to_string() {
					let _ = sqlx::query_as!(
						PageBlockSection,
						r#"INSERT INTO pages_blocks_sections (page_block_id, page_block_section_order, text) VALUES ($1, $2, $3) RETURNING *;"#,
        	prices_block_id,
         	index.to_string(),
						key.replace("/n", ""),
					)
					.fetch_one(&pool)
					.await?;
				}
			}

			// === VIN ===
			if cur_case.vin.is_some() && cur_case.vin.clone().expect("vin") != "".to_string()  {
				let tags_block = sqlx::query_as!(
					PageBlock,
					r#"INSERT INTO pages_blocks (page_id, page_block_order, page_block_title, page_block_subtitle, page_block_type) VALUES ($1, $2, $3, $4, $5) RETURNING *"#,
					cur_page_id,
					"5".to_string(),
					"VIN номер:".to_string(),
					cur_case.vin.unwrap_or("".to_string()),
					0,
				)
				.fetch_one(&pool)
				.await?;
			}

			// === TAGS ===
			let tags_block = sqlx::query_as!(
				PageBlock,
				r#"INSERT INTO pages_blocks (page_id, page_block_order, page_block_title, page_block_type) VALUES ($1, $2, $3, $4) RETURNING *"#,
				cur_page_id,
				"6".to_string(),
				"Смотрите также:".to_string(),
				2,
			)
			.fetch_one(&pool)
			.await?;

			let tags_block_id = tags_block.clone().page_block_id;
			dbg!(&tags_block_id);

			let tags_preamble = format!(
				"
				The Text:
				{}
				",
				&case_description
			);

			let tags_body = json!({
			  "model": model_name,
				"stream": false,
			  "messages": [
					{
						"role": "system",
						"content": format!("
						1. Act as a professional SEO specialist/SEO writer.
						2. Context: I will provide you with the Text.
						3. Your task: Generate tags (keyphrases) for SEO promotion of a page: {} headlight repair. Generate 5 Main keyphrases (high frequency). Generate 5 Additional keyphrases (mid- and low-frequency). Generate 5 Technical and LSI keyphrases (to enhance relevance).
						4. Format: Write your answer only in the Russian language, do not use Chinese words/hieroglyphs or English words/letters or other languages, only Russian language. Give the answer in listicle form. Each item from a new line. Don't write what you think and don't use any system phrases in your answer. Do not use phrases like: *Ответ:*
						5. Think step by step.",
						case_name.clone().unwrap_or("".to_string()))
					},
					{
						"role": "user",
						"content": &tags_preamble.replace("\t", "").replace("\n", "")
					}
				]
			});

			// request
			let tags_response = reqwest::Client::builder()
				.danger_accept_invalid_certs(true)
				.build()
				.unwrap()
				.post(url.clone())
				.headers(headers.clone())
				.json(&tags_body)
				.send()
				.await?;

			let empty_message = Message {
				content: "".to_string(),
			};

			let tags_res: ApiResponse = match tags_response.json().await {
				Ok(result) => result,
				Err(e) => {
					println!("Network error: {:?}", e);
					ApiResponse {
						message: empty_message,
					}
				}
			};

			let tags_oai_res = &tags_res.message.content;

			// response
			println!("============");
			println!("{:?}", &tags_oai_res);

			if tags_oai_res == "" {
				continue;
			}

			let mut tags_oai_array = tags_oai_res
				.split(&split_target)
				.filter(|&x| *x != *"   ")
				.collect::<Vec<&str>>();

			for (index, key) in tags_oai_array.iter().enumerate() {
				if *key != "".to_string() && *key != " ".to_string() {
					let _ = sqlx::query_as!(
						PageBlockSection,
						r#"INSERT INTO pages_blocks_sections (page_block_id, page_block_section_order, text, url) VALUES ($1, $2, $3, $4) RETURNING *;"#,
        	tags_block_id,
         	index.to_string(),
					key.replace("/n", ""),
					&encode(&prepared_firm_url.as_str()),
					)
					.fetch_one(&pool)
					.await?;
				}
			}
		}
	}

	Ok(())
}
