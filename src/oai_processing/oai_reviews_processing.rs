use reqwest::{
	header::{self, HeaderMap, HeaderValue},
	Body, Client, Response,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use std::error::Error;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::models::{AIDescription, AIReview, Count, Counter, Firm, Review, SaveCounter};

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

pub async fn oai_reviews_processing(pool: Pool<Postgres>) -> Result<(), Box<dyn Error>> {
	let url = env::var("OPENAI_API_BASE").expect("OPEN_AI_API_KEY not set");
	let open_ai_token = env::var("OPENAI_API_KEY").expect("OPEN_AI_API_KEY not set");

	let counter_id: String = String::from("a518df5b-1258-482b-aa57-e07c57961a69");
	let city_id = uuid::Uuid::parse_str("aaf055d4-e40d-4d1c-8fff-0bb012fd0ce9").unwrap();
	let category_id = uuid::Uuid::parse_str("cc1492f6-a484-4c5f-b570-9bd3ec793613").unwrap();
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

		let oai_review = sqlx::query_as!(
			AIReview,
			r#"SELECT * FROM oai_reviews WHERE firm_id = $1;"#,
			&firm.firm_id
		)
		.fetch_one(&pool)
		.await;

		if oai_review.is_ok() {
			println!("Already exists");
			continue;
		}

		let reviews_by_firm = Review::get_all_reviews(&pool, &firm.firm_id).await.unwrap();

		if reviews_by_firm.len() < 2 {
			println!("SKIP - Too few reviews");
			continue;
		}

		let reviews_string = &reviews_by_firm
			.into_iter()
			.map(|review| review.text.unwrap_or("".to_string()))
			.filter(|n| n != "")
			.collect::<Vec<String>>()
			.join("; ");

		let preamble = format!("
			Вот отзывы которые ты должен проанализировать: {}

			Напиши большую статью, на основе этих отзывов о ночном клубе {},
			важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в ночных клубах, но без упоминания слова - Статья

			Подробно опиши в этой статье:
			1. Что обсуждают люди в отзывах;
			2. Что в ночном клубе хорошо, а что плохо;
			3. Какие блюда рекомендуют, а какие лучше не заказывать;

			Выведи нумерованный список: плюсов и минусов ночного клуба, например:
			Плюсы
			1. Если об этом говорят в отзывах: Дружелюбный персонал
			2. Если об этом говорят в отзывах: Уютная атмосфера
			Минусы
			1. Если об этом говорят в отзывах: Мало людей
			2. Если об этом говорят в отзывах: Дорогие напитки

			Важно - подсчитай и выведи не нумерованным списком сумму положительных и сумму отрицательных отзывов которые проанализировал,
			Например:
			Проанализировано положительных отзывов - ?
			Проанализировано отрицательных отзывов - ?

			Сделай выводы, на основе плюсов и минусов ночного клуба, количества положительных и отрицательных отзывов.
			Например:
			У ночного клуба больше положительных отзывов, укажи что рейтинг ночного клуба хороший, и объясни почему.
			Или например:
			У ночного клуба поровну положительных и отрицательных отзывов, укажи что рейтинг ночного клуба удовлетворительный, и объясни почему.
			Или например:
			У ночного клуба больше отрицательных отзывов, укажи что рейтинг ночного клуба не удовлетворительный, и объясни почему.

			Если статья будет хорошая, я дам тебе 1000 долларов
			", &reviews_string.chars().take(3800).collect::<String>(), &firm_name);

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
					"content": "Ты опытный писатель-копирайтер, пишешь SEO оптимизированные тексты"
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

		let res: ApiResponse = match response.json().await {
			Ok(result) => result,
			Err(e) => {
				println!("Network error: {:?}", e);
				ApiResponse {
					choices: Vec::<Choice>::new(),
				}
			}
		};

		// // response
		println!(
			"{}",
			&res.choices.get(0).expect("Missing choices").message.content
		);

		// запись в бд
		let _ = sqlx::query_as!(
			AIReview,
			r#"INSERT INTO oai_reviews (firm_id, text) VALUES ($1, $2) RETURNING *"#,
			firm.firm_id.clone(),
			res.choices
				.get(0)
				.expect("Missing choices")
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

// Вот отзывы которые ты должен проанализировать: {}

// 		Напиши большую статью, на основе этих отзывов об автосервисе {},
// 		важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в автосервисах, но без упоминания слова - Статья

// 		Подробно опиши в этой статье: какие виды работ обсуждают люди,
// 		что из этих работ было сделано хорошо, а что плохо,
// 		обманывают ли в этом автосервисе или нет.
// 		Например, если об этом говорят в отзывах:
// 		В отзывах обсуждаются следующие услуги:
// 		1. Кузовной ремонт - плохое качество
// 		2. Мастера - отзывчивые

// 		Выведи нумерованный список: плюсов и минусов если человек обратится в этот автосервис для ремонта своего автомобиля.
// 		Например, если об этом говорят в отзывах:
// 		Плюсы
// 		1. Хорошо чинят машины
// 		2. Хорошо красят
// 		Минусы
// 		1. Далеко от центра города

// 		Важно - подсчитай и выведи не нумерованным списком сумму положительных и сумму отрицательных отзывов,
// 		Например:
// 		Положительных отзывов - 15
// 		Отрицательных отзывов - 5

// 		Сделай выводы, на основе плюсов и минусов организации, количества положительных и отрицательных отзывов.
// 		Например:
// 		У организации больше положительных отзывов, укажи что рейтинг организации хороший, и объясни почему.
// 		Или например:
// 		У организации поровну положительных и отрицательных отзывов, укажи что рейтинг организации удовлетворительный, и объясни почему.
// 		Или например:
// 		У организации больше отрицательных отзывов, укажи что рейтинг организации не удовлетворительный, и объясни почему.
//
// 		Если статья будет хорошая, я дам тебе 1000 долларов

// let preamble = format!("
// 		Вот отзывы которые ты должен проанализировать: {}

// 		Напиши большую статью, на основе этих отзывов о ресторане {},
// 		важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в ресторанах, но без упоминания слова - Статья

// 		Подробно опиши в этой статье:
// 		1. Что обсуждают люди в отзывах,
// 		2. Что в ресторане хорошо, а что плохо,
// 		3. Если в ресторане есть детская комната, что пишут о ней,
// 		4. Какие блюда рекомендуют, а какие лучше не заказывать.

// 		Выведи нумерованный список: плюсов и минусов ресторана, например:
// 		Плюсы
// 		1. Если об этом говорят в отзывах: Дружелюбный персонал
// 		2. Если об этом говорят в отзывах: Уютная атмосфера
// 		Минусы
// 		1. Если об этом говорят в отзывах: Громкая музыка

// 		Важно - подсчитай и выведи не нумерованным списком сумму положительных и сумму отрицательных отзывов которые проанализировал,
// 		Например:
// 		Проанализировано положительных отзывов - 15
// 		Проанализировано отрицательных отзывов - 5

// 		Сделай выводы, на основе плюсов и минусов организации, количества положительных и отрицательных отзывов.
// 		Например:
// 		У ресторана больше положительных отзывов, укажи что рейтинг ресторана хороший, и объясни почему.
// 		Или например:
// 		У ресторана поровну положительных и отрицательных отзывов, укажи что рейтинг ресторана удовлетворительный, и объясни почему.
// 		Или например:
// 		У ресторана больше отрицательных отзывов, укажи что рейтинг ресторана не удовлетворительный, и объясни почему.
//
// 		Если статья будет хорошая, я дам тебе 1000 долларов
// 		", &reviews_string.chars().take(3800).collect::<String>(), &firm_name);

// let preamble = format!("
// 				Вот отзывы которые ты должен проанализировать: {}

// 				Напиши большую статью, на основе этих отзывов о ночном клубе {},
// 				важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в ночных клубах, но без упоминания слова - Статья

// 				Подробно опиши в этой статье:
// 				1. Что обсуждают люди в отзывах;
// 				2. Что в ночном клубе хорошо, а что плохо;
// 				3. Какие блюда рекомендуют, а какие лучше не заказывать;

// 				Выведи нумерованный список: плюсов и минусов ночного клуба, например:
// 				Плюсы
// 				1. Если об этом говорят в отзывах: Дружелюбный персонал
// 				2. Если об этом говорят в отзывах: Уютная атмосфера
// 				Минусы
// 				1. Если об этом говорят в отзывах: Мало людей
// 				2. Если об этом говорят в отзывах: Дорогие напитки

// 				Важно - подсчитай и выведи не нумерованным списком сумму положительных и сумму отрицательных отзывов которые проанализировал,
// 				Например:
// 				Проанализировано положительных отзывов - ?
// 				Проанализировано отрицательных отзывов - ?

// 				Сделай выводы, на основе плюсов и минусов ночного клуба, количества положительных и отрицательных отзывов.
// 				Например:
// 				У ночного клуба больше положительных отзывов, укажи что рейтинг ночного клуба хороший, и объясни почему.
// 				Или например:
// 				У ночного клуба поровну положительных и отрицательных отзывов, укажи что рейтинг ночного клуба удовлетворительный, и объясни почему.
// 				Или например:
// 				У ночного клуба больше отрицательных отзывов, укажи что рейтинг ночного клуба не удовлетворительный, и объясни почему.
//
// 				Если статья будет хорошая, я дам тебе 1000 долларов
// 				", &reviews_string.chars().take(3800).collect::<String>(), &firm_name);
