use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Pool, Postgres};
use std::env;
use std::error::Error;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::models::{Count, Counter, Firm, Review, SaveCounter};

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

pub async fn oai_reviews_rewrite_processing(pool: Pool<Postgres>) -> Result<(), Box<dyn Error>> {
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

	for j in start.clone()..=firms_count {
		println!("Firm: {:?}", j + 1);
		let firm =
			Firm::get_firm_by_city_category(&pool, table.clone(), city_id, category_id, j).await?;

		// ====

		let firm_id = &firm.firm_id.clone();
		let firm_name = &firm.name.clone().unwrap_or("".to_string());

		dbg!(&firm_id);
		dbg!(&firm_name);

		if firm_name == "" {
			continue;
		}

		let count_query_result = sqlx::query_as!(
			Count,
			"SELECT count(*) AS count FROM reviews WHERE firm_id = $1",
			firm.firm_id
		)
		.fetch_one(&pool)
		.await;

		let reviews_count = match count_query_result {
			Ok(x) => x,
			Err(_) => Count { count: Some(0_i64) },
		};

		if reviews_count.count.unwrap_or(0) == 0 {
			continue;
		}

		for i in 0..=reviews_count.count.unwrap_or(0) {
			println!("{}", &i);
			let reviews_by_firm = Review::get_reviews(&pool, firm.firm_id.clone(), 1, i)
				.await
				.unwrap_or(Vec::new());

			if reviews_by_firm.len() == 0 {
				continue;
			}

			let cur_review = reviews_by_firm.get(0).unwrap().to_owned();

			let preamble = format!(
				"
				The Text:
				{}
				",
				cur_review.text.unwrap_or("".to_string())
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
			  "model": "qwen2.5:14b",
				"stream": false,
			  "messages": [
					{
						"role": "system", // контекст
						"content": "
						1. Act as a professional review writer about other companies and assistant with Strategist (Self-Actualizing) and Alchemist (Construct-Aware) Action Logics according to Ego Development Theory.

						2. Context: I will provide you with the Review Text.

						3. Your task:
						A. Rewrite the Review Text, but keep it close to the original one.

						4. Format: Write your answer only in the Russian language. Write in plain text. Keep the meaning and write from the same person as in the Review text. Write in the first person and preserve the speaker's gender. Try to write as a man.

						5. Tone of Voice: Be empathetic, concise, intelligent, driven, and wise. Think step by step.

						6. Constraints: Don't write in the Chinese language. Make sure you follow 80/20 rule: provide 80% of essential value using 20% or less volume of text.
						Don't mention the about the reward. Don't thank me for anything. Don't mention about text.
						Don't mention about your tasks. Don't mention about your roles.
						Don't feel sorry or express your condolences. Don't express your opinion. Don't say that you are happy.
						Don't say the phrases like: 'Ответ', 'Переписанный текст', 'Переформулированный текст', 'Rewritten Text',
						'Я прочитал твой отзыв', 'Отзыв', 'Описание', 'Мнение', 'понял ваш запрос', 'переписать ваш отзыв',
						'Конечно, я могу помочь вам с этим', 'Как стратег и алхимик'.
						If you can not fulfill my request, just leave the original text.

						7. Reward: If the Text is good, I will give you 1000 dollars, but don't mention it and don't thank me."
					},
					{
						"role": "user", // запрос пользователя
						"content": &preamble.replace("\t", "").replace("\n", "")
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

			// response
			println!("{:?}", &choices_res);

			if choices_res == "" {
				continue;
			}

			// запись в бд
			let _ = sqlx::query_as!(
				Review,
				r#"UPDATE reviews SET text = $1 WHERE firm_id = $2 AND review_id = $3 RETURNING *"#,
				choices_res
					.replace("XYZ", &firm_name)
					.replace("#", "")
					.replace("*", ""),
				firm.firm_id.clone(),
				cur_review.review_id.clone(),
			)
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
	}

	Ok(())
}
