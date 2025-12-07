use chrono::{DateTime, TimeZone, Utc};
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Pool, Postgres};
use std::env;
use std::error::Error;
use tokio::time::{sleep, Duration};
use urlencoding::encode;
use uuid::Uuid;

use crate::models::{AIDescription, Count, Counter, Firm, Review, SaveCounter};

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

pub async fn title_processing(pool: Pool<Postgres>) -> Result<(), Box<dyn std::error::Error>> {
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
	let url = env::var("OPENAI_API_BASE").expect("OPEN_AI_API_KEY not set");
	let open_ai_token = env::var("OPENAI_API_KEY").expect("OPEN_AI_API_KEY not set");
	let counter_id: String = String::from("759f3b92-4d38-4980-a18f-ada6302e75b8");
	let model_name: String = String::from("deepseek-v2:16b");

	// let firms_count =
	// 	Count::count_firms_with_empty_field(&pool, table.clone(), "title".to_string())
	// 		.await
	// 		.unwrap_or(0);

	let firms_count = Count::count_firms_by_city_category(
		&pool,
		table.clone(),
		city_id.clone(),
		category_id.clone(),
	)
	.await
	.unwrap_or(0);

	// получаем из базы начало счетчика
	let start = Counter::get_counter(&pool, &counter_id)
		.await?
		.value
		.unwrap_or(String::from("0"))
		.parse::<i64>()
		.unwrap_or(0);

	for j in start.clone()..=firms_count {
		println!("№ {}", &j);
		// let firm = Firm::get_firm_with_empty_field(&pool, table.clone(), "title".to_string(), j)
		// 	.await
		// 	.unwrap();

		let firm = Firm::get_firm_by_city_category(&pool, table.clone(), city_id, category_id, j)
			.await
			.unwrap();

		println!("Firm {}", &firm.firm_id.clone());

		let oai_description_result =
			AIDescription::get_oai_descriptions(&pool, firm.firm_id.clone())
				.await
				.unwrap_or(Vec::new());

		let oai_description_res = oai_description_result
			.get(0)
			.unwrap_or(&AIDescription {
				oai_description_id: uuid::Uuid::new_v4(),
				oai_description_value: Some("".to_string()),
				firm_id: firm.firm_id.clone(),
				created_ts: Some(DateTime::from_timestamp(61, 0).unwrap()),
				updated_ts: Some(DateTime::from_timestamp(61, 0).unwrap()),
			})
			.to_owned();

		let ai_description = if &oai_description_res
			.oai_description_value
			.clone()
			.unwrap_or("".to_string())
			!= ""
		{
			&oai_description_res
				.oai_description_value
				.clone()
				.unwrap_or("".to_string())
		} else {
			&firm
				.description
				.clone()
				.unwrap_or("".to_string())
				.to_string()
		};

		let firm_address = firm.address.clone().unwrap_or("".to_string());
		let firm_street = firm_address.split(",").collect::<Vec<&str>>()[0].to_string();
		let firm_house = firm_address.split(",").collect::<Vec<&str>>()[1].to_string();
		let address_string = if firm_address != "" {
			format!("{} {}", firm_street, firm_house)
		} else {
			firm.firm_id.clone().to_string()
		};

		let mut firm_title = String::new();

		let firms_double_titles: Vec<String> = Vec::new();
		// sqlx::query_as::<_, Firm>(r#"SELECT * FROM firms WHERE title = $1"#)
		// 	.bind(&firm.title.clone().unwrap_or("".to_string()))
		// 	.fetch_all(&pool)
		// 	.await?;

		if firms_double_titles.len() > 0 {
			firm_title = format!(
				"Автосервис {} | {}-{}",
				&firm.name.clone().unwrap(),
				&address_string,
				&firm.firm_id.clone()
			);
		} else {
			firm_title = format!(
				"Автосервис {} | {}",
				&firm.name.clone().unwrap(),
				&address_string
			);
		}

		let preamble = format!(
			"
			The Text:
			{}
			",
			format!("{}, {}", &firm.name.clone().unwrap(), &ai_description)
		);

		let headers: HeaderMap<HeaderValue> = header::HeaderMap::from_iter(vec![
			(header::ACCEPT, "application/json".parse().unwrap()),
			(header::CONTENT_TYPE, "application/json".parse().unwrap()),
			// (
			// 	header::AUTHORIZATION,
			// 	format!("Bearer {}", open_ai_token).parse().unwrap(),
			// ),
		]);

		let body = json!({
		  "model": model_name,
			"stream": false,
		  "messages": [
				{
					"role": "system", // контекст
					"content": "
					1. Act as a professional SEO specialist and writer about and assistant with Strategist (Self-Actualizing) and Alchemist (Construct-Aware) Action Logics according to Ego Development Theory.

					2. Context: I will provide you with the Text.

					3. Your task:
					A. Generate the best SEO Title, for web page about organization. Example:  Автосервис АВТОДОМ BRP - быстрый и надежный ремонт | Опытные мастера | Гарантия качества

					4. Format: Write your answer only in the Russian language. Title must be 100 symbols length maximum.

					5. Tone of Voice: Be empathetic, concise, intelligent, driven, and wise. Think step by step.

					6. Constraints:
					Text must be 100 symbols length maximum.
					Don't write in the Chinese language.
					Don't translate the name of the organization.
					Don't mention the about the reward. Don't thank me for anything. Don't mention about text. Don't use the symbols \".
					Don't mention about your tasks. Don't mention about your roles.
					Don't feel sorry or express your condolences. Don't express your opinion. Don't say that you are happy.
					Don't say the phrases like: 'Ответ', 'Переписанный текст', 'Переформулированный текст', 'Rewritten Text',
					'Я прочитал твой текст', 'Отзыв', 'Описание', 'Мнение', 'понял ваш запрос', 'переписать ваш отзыв',
					'Конечно, я могу помочь вам с этим', 'Как стратег и алхимик'.
					If you can not fulfill my request, just leave the original text.

					7. Reward: If the Text is good, I will give you 1000 dollars, but don't mention it and don't thank me."
				},
				{
					"role": "user", // запрос пользователя
					"content": &preamble.replace("\t", "").replace("\n", "").replace("\u{200b}", " ").replace("  ", " ")
				}
			]
		});

		// request
		let response = reqwest::Client::builder()
			.danger_accept_invalid_certs(true)
			.build()
			.unwrap()
			.post(url.clone())
			.headers(headers)
			.json(&body)
			.send()
			.await?;

		let empty_message = Message {
			content: "".to_string(),
		};

		let res: ApiResponse = match response.json().await {
			Ok(result) => result,
			Err(e) => {
				println!("Network error: {:?}", e);
				ApiResponse {
					message: empty_message,
				}
			}
		};

		// let empty_choices = Choice {
		// 	message: empty_message,
		// };

		// let choices_res = &res.choices.get(0).unwrap_or(&empty_choices).message.content;
		let choices_res = &res.message.content;

		let title = format!(
			"{} {}",
			&firm_title
				.replace("`", "")
				.replace("/", "-")
				.replace("&amp;", "&")
				.replace("--", "-")
				.replace("\"", "")
				.replace("\u{200b}", " ")
				.replace("\u{fe0f}", " ")
				.replace("  ", " ")
				.as_str(),
			format!(
				"| {}",
				&choices_res
					.replace("`", "")
					.replace("/", "-")
					.replace("&amp;", "&")
					.replace("--", "-")
					.replace("\"", "")
					.replace("\"", "")
					.replace("\u{200b}", " ")
					.replace("\u{fe0f}", " ")
					.replace("  ", " ")
					.as_str()
			)
		);

		// response
		println!("{}", &title);

		let _ = sqlx::query_as::<_, Firm>(
			r#"UPDATE firms SET title = $1 WHERE firm_id = $2 RETURNING *"#,
		)
		.bind(&title)
		.bind(firm.firm_id)
		.fetch_one(&pool)
		.await;

		let _ = Counter::update_counter(
			&pool,
			SaveCounter {
				counter_id: Uuid::parse_str(&counter_id).unwrap(),
				value: (j + 1).to_string(),
				city_id: city_id.to_string(),
				category_id: category_id.to_string(),
			},
		)
		.await;
	}

	Ok(())
}
