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

#[allow(unreachable_code)]
pub async fn oai_description_processing(
	pool: Pool<Postgres>,
	// _: jwt_auth::JwtMiddleware,
) -> Result<(), Box<dyn Error>> {
	loop {
		let mut needs_to_restart = true;
		if needs_to_restart {
			let _: Result<(), Box<dyn std::error::Error>> = match processing(pool.clone()).await {
				Ok(x) => {
					needs_to_restart = false;
					Ok(x)
				}
				Err(e) => {
					println!("{:?}", e);
					let _ = sleep(Duration::from_secs(20)).await;
					needs_to_restart = true;
					Err(e)
				}
			};
		}
	}

	Ok(())
}

async fn processing(pool: Pool<Postgres>) -> Result<(), Box<dyn Error>> {
	let url = env::var("OPENAI_API_BASE").expect("OPEN_AI_API_KEY not set");
	let open_ai_token = env::var("OPENAI_API_KEY").expect("OPEN_AI_API_KEY not set");

	let counter_id: String = String::from("5e4f8432-c1db-4980-9b63-127fd320cdde");
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

		let preamble = format!("
			The Text:
				{}.
				Добро пожаловать в наш салон цветов! Мы предлагаем широкий ассортимент услуг и композиций, которые помогут вам окружить себя красотой и подарить радость близким. Наша команда профессиональных флористов обладает не только мастерством создания изысканных букетов, но и индивидуальным подходом к каждому клиенту.
				У нас вы найдете самые свежие и качественные цветы, оригинальные композиции и уникальные декоративные элементы, созданные с душой. Мы уверены, что цветы способны передать самые искренние эмоции, поэтому стремимся к безупречному качеству во всем, что делаем.
				С нами вы ощутите гармонию и вдохновение, окружив себя красотой природы. Наша команда всегда готова помочь вам выбрать идеальный букет или создать эксклюзивную композицию, которая подчеркнет ваш стиль и настроение. Мы работаем для того, чтобы вы могли радовать себя и своих близких, окружая жизнь яркими и живыми красками!
				{}
				", &firm_name, &firm_desc);

		let headers: HeaderMap<HeaderValue> = header::HeaderMap::from_iter(vec![
			(header::ACCEPT, "application/json".parse().unwrap()),
			(header::CONTENT_TYPE, "application/json".parse().unwrap()),
			(
				header::AUTHORIZATION,
				format!("Bearer {}", open_ai_token).parse().unwrap(),
			),
		]);

		let body = json!({
		  // "model": "gpt-3.5-turbo",
		  "model": "gpt-4o-mini",
		  "messages": [
				{
					"role": "system", // контекст
					"content": "
					1. Act as a professional writer and assistant with Strategist (Self-Actualizing) and Alchemist (Construct-Aware) Action Logics according to Ego Development Theory.

					2. Context: I will provide you with the Text.

					3. Your task:
					A. Rewrite the Text, rephrase and expand it.
					B. Add up to 3 key items summerizing the text.

					4. Format: Write your answer ONLY in the Russian language. Write in plain text.

					5. Tone of Voice: Be empathetic, concise, intelligent, driven, and wise. Think step by step.

					6. Constraints: Make sure you follow 80/20 rule: provide 80% of essential value using 20% or less volume of text.
					Do not mention the about the reward. Do not thank me for anything. Do not mention about text.
					Do not mention about your tasks. Do not mention about your roles. Do not mention the phrase 'Ответ'.
					Do not mention the phrase 'Переписанный текст'. Do not mention the phrase 'Переформулированный текст'

					7. Reward: If the Text is good, I will give you 1000 dollars.
					"
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
				city_id: city_id.to_string(),
				category_id: category_id.to_string(),
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
//
// let preamble = format!("Вот описание ресторана которое ты должен проанализировать: {}

// Напиши большую статью о ресторане, на основе анализа этого описания {},
// важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в ресторанах, но без упоминания слова - \"Статья\"

// Подробно опиши в этой статье:
// 1. На чем специализируется данный ресторан, например, если об этом указано в описании:

// Данный ресторан специализируется на европейской кухне

// 2. Придумай в чем заключается миссия данного ресторана, чем он помогает людям.

// 3. Укажи что в ресторане работают опытные и квалифицированные сотрудники, которые всегда помогут и сделают это быстро и качественно.

// 4. В конце текста укажи: Для получения более детальной информации позвоните по номеру: {}

// И перечисли все виды блюд, которые могут быть приготовлены в данном ресторане

// Если статья будет хорошая, я дам тебе 100 рублей
// ", &firm_desc, &firm_name, &firm_phone);

// let preamble = format!("
// 		Проанализируй следующее описание школы {}, перефарзируй его и расширь, сохраняя основную идею о чем в нем говорится. Укажи отдельно положительные аспекты.

// 		Важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в школых, но без упоминания слова - \"Статья\"
// 		Не задавай уточняющих вопросов.
// 		Не благодари за предоставленную информацию.

// 		Вот описание школы которое ты должен проанализировать: {}

// 		Если описания не достаточно, не задавай вопросы, а просто ответь: Недостаточно информации

// 		Подробно опиши в этой статье:
// 		1. Что в школе сильный преподавательский состав, если об этом указано в описании:

// 		2. Придумай в чем заключается миссия данного школы, чем она помогает ученикам и кем они смогут стать в дальнейшем.

// 		3. Придумай в чем заключается уникальный подход к обучению в этой школе

// 		4. В конце текста укажи: Для получения более детальной информации позвоните по номеру: {}

// 		Если статья будет хорошая, я дам тебе 1000 долларов, но не упоминай об этом
// 		", &firm_name, &firm_desc, &firm_phone);
