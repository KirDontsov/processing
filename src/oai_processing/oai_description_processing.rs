use reqwest::{
	header::{self, HeaderMap, HeaderValue},
	Body, Client,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use std::error::Error;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::models::{AIDescription, Count, Counter, Firm, SaveCounter, UpdateFirmDesc};

#[derive(Debug, Deserialize, Serialize)]
struct AccessToken {
	access_token: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ApiResponse {
	choices: Vec<Choice>,
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

pub async fn oai_description_processing(pool: Pool<Postgres>) -> Result<(), Box<dyn Error>> {
	let url = env::var("OPENAI_API_BASE").expect("OPEN_AI_API_KEY not set");
	let open_ai_token = env::var("OPENAI_API_KEY").expect("OPEN_AI_API_KEY not set");

	let counter_id: String = String::from("5e4f8432-c1db-4980-9b63-127fd320cdde");
	let city_id = uuid::Uuid::parse_str("9c846039-0f8e-4a05-8083-cc5cdf7aa89f").unwrap();
	let category_id = uuid::Uuid::parse_str("3ebc7206-6fed-4ea7-a000-27a74e867c9a").unwrap();
	let city = "nabchelny";
	let category_name = "рестораны";
	let rubric_id = "9041";
	let table = String::from("firms");

	let query_uuid = Uuid::new_v4();

	// получаем из базы кол-во фирм
	let firms_count =
		Count::count_firms_by_city_category(&pool, table.clone(), city_id, category_id)
			.await
			.unwrap_or(0);

	dbg!(&firms_count);

	// получаем из базы начало счетчика
	let start = Counter::get_counter(&pool, &counter_id)
		.await?
		.value
		.unwrap_or(String::from("0"))
		.parse::<i64>()
		.unwrap_or(0);

	for j in start.clone()..firms_count {
		println!("Firm: {:?}", j + 1);
		let firm =
			Firm::get_firm_by_city_category(&pool, table.clone(), city_id, category_id, j).await?;

		// ====

		let firm_id = &firm.firm_id.clone();
		let firm_name = &firm.name.clone().unwrap_or("".to_string());
		let firm_desc = &firm.description.clone().unwrap_or("".to_string());
		let firm_phone = &firm.default_phone.clone().unwrap_or("".to_string());
		dbg!(&firm_id);
		dbg!(&firm_name);

		if firm_name == "" || firm_desc == "" {
			continue;
		}

		let oai_description = sqlx::query_as!(
			AIDescription,
			r#"SELECT * FROM oai_descriptions WHERE firm_id = $1;"#,
			&firm.firm_id
		)
		.fetch_one(&pool)
		.await;

		if oai_description.is_ok() {
			println!("Already exists");
			continue;
		}
		if firm_desc.clone() == "" {
			println!("Empty description");
			continue;
		}

		let preamble = format!("Вот описание ресторана которое ты должен проанализировать: {}

				Напиши большую статью о ресторане, на основе анализа этого описания {},
				важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в ресторанах, но без упоминания слова - \"Статья\"

				Подробно опиши в этой статье:
				1. На чем специализируется данный ресторан, например, если об этом указано в описании:

				Данный ресторан специализируется на европейской кухне

				2. Придумай в чем заключается миссия данного ресторана, чем он помогает людям.

				3. Укажи что в ресторане работают опытные и квалифицированные сотрудники, которые всегда помогут и сделают это быстро и качественно.

				4. В конце текста укажи: Для получения более детальной информации позвоните по номеру: {}

				И перечисли все виды блюд, которые могут быть приготовлены в данном ресторане

				Если статья будет хорошая, я дам тебе 100 рублей
				", &firm_desc, &firm_name, &firm_phone);

		let headers: HeaderMap<HeaderValue> = header::HeaderMap::from_iter(vec![
			(header::ACCEPT, "application/json".parse().unwrap()),
			(header::CONTENT_TYPE, "application/json".parse().unwrap()),
			(
				header::AUTHORIZATION,
				format!("Bearer {}", open_ai_token).parse().unwrap(),
			),
		]);

		let body = json!({
		  "model": "gpt-3.5-turbo", // идентификатор модели, можно указать конкретную или :latest  для выбора наиболее актуальной
		  "messages": [
				{
					"role": "system", // контекст
					"content": "Отвечай как опытный копирайтер"
				},
				{
					"role": "user", // запрос пользователя
					"content": &preamble.replace("\t", "").replace("\n", "")
				}
			]
		});

		// request
		let response: ApiResponse = reqwest::Client::builder()
			.danger_accept_invalid_certs(true)
			.build()
			.unwrap()
			.post(url.clone())
			.headers(headers)
			.json(&body)
			.send()
			.await?
			.json()
			.await?;

		// // response
		println!("{}", &response.choices[0].message.content);

		// запись в бд
		let _ = sqlx::query_as!(
			AIDescription,
			r#"INSERT INTO oai_descriptions (firm_id, oai_description_value) VALUES ($1, $2) RETURNING *"#,
			firm.firm_id.clone(),
			response.choices[0]
				.message
				.content
				.replace("XYZ", &firm_name)
				.replace("#", "")
				.replace("*", ""),
		)
		.fetch_one(&pool)
		.await;

		let _ = Counter::update_counter(
			&pool,
			SaveCounter {
				counter_id: Uuid::parse_str(&counter_id).unwrap(),
				value: (j + 1).to_string(),
			},
		)
		.await;
	}

	Ok(())
}

// let preamble = format!("Вот описание автосервиса которое ты должен проанализировать: {}

// 				Напиши большую статью об автосервисе, на основе анализа этого описания {},
// 				важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в автосервисах, но без упоминания слова - \"Статья\"

// 				Подробно опиши в этой статье:
// 				1. Какие виды работ может осуществлять данная организация, например, если об этом указано в описании:
// 				Данная организация может оказывать следующие виды работ: Кузовной ремонт, Замена масла, Замена шин, Покраска

// 				2. Придумай в чем заключается миссия данной организации по ремонту автомобилей, чем она помогает людям.

// 				3. Укажи что в компании работают опытные и квалифицированные сотрудники, которые всегда помогут и сделают это быстро и качественно.

// 				4. В конце текста укажи: Для получения более детальной информации позвоните по номеру: {} (если он указан)

// 				5. И перечисли все виды работ, которые могут быть свзаны с ремонтом автомобиля
// 				", &firm_desc, &firm_name, &firm_phone);
