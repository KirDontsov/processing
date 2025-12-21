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

pub async fn oai_reviews_processing(
	pool: Pool<Postgres>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
	let url = env::var("OPENAI_API_BASE").expect("OPEN_AI_API_KEY not set");
	let open_ai_token = env::var("OPENAI_API_KEY").expect("OPEN_AI_API_KEY not set");

	let counter_id: String = String::from("a518df5b-1258-482b-aa57-e07c57961a69");
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
		.await
		.map_err(|e| {
			Box::new(std::io::Error::new(
				std::io::ErrorKind::Other,
				format!("{}", e),
			)) as Box<dyn std::error::Error + Send + Sync>
		})?
		.value
		.unwrap_or(String::from("0"))
		.parse::<i64>()
		.unwrap_or(0);

	for j in start.clone()..firms_count {
		println!("Firm: {:?}", j + 1);
		let firm = Firm::get_firm_by_city_category(&pool, table.clone(), city_id, category_id, j)
			.await
			.map_err(|e| {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("{}", e),
				)) as Box<dyn std::error::Error + Send + Sync>
			})?;

		// ====

		let firm_id = &firm.firm_id.clone();
		let firm_name = &firm.name.clone().unwrap_or("".to_string());
		dbg!(&firm_id);
		dbg!(&firm_name);

		if firm_name == "" {
			continue;
		}

		let oai_review: Result<AIReview, sqlx::Error> = sqlx::query_as!(
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

		let reviews_by_firm = Review::get_all_reviews(&pool, &firm.firm_id)
			.await
			.map_err(|e| {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("{}", e),
				)) as Box<dyn std::error::Error + Send + Sync>
			})?;

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

		let preamble = format!(
			"
			The Text:
			{}
			",
			&reviews_string.chars().take(3800).collect::<String>()
		);

		let headers: HeaderMap<HeaderValue> = header::HeaderMap::from_iter(vec![
			(header::ACCEPT, "application/json".parse().unwrap()),
			(header::CONTENT_TYPE, "application/json".parse().unwrap()),
			(
				header::AUTHORIZATION,
				format!("Bearer {}", open_ai_token).parse().unwrap(),
			),
		]);

		let body = json!({
		  "model": "gpt-4o-mini", // идентификатор модели, можно указать конкретную или :latest для выбора наиболее актуальной
		  "messages": [
				{
					"role": "system", // контекст
					"content": "
					1. Act as a professional summarizer and assistant with Strategist  (Self-Actualizing) and Alchemist (Construct-Aware) Action Logics according to Ego Development Theory.
					2. Context: I will provide you with the reviews Text.
					3. Your task:
					A. Analyze and summarize key points of the reviews Text into 3-5 bullet points and add general positive and negative aspects.
					B. Output a numbered list: the pros and cons of the company, for example:
					Pros:
					– Good and experienced professionals - If they say so in the reviews
					– Cleanliness - If they say so in the reviews:
					Cons:
					– Old flowers - If they say so in the reviews
					– Foreign odors - If they say so in the reviews
					C. Count and output in an unnumbered list the sum of positive and the sum of negative reviews that you analyzed.
					For example:
					Positive reviews analyzed - X
					Negative reviews analyzed - X
					D. Draw conclusions based on the pros and cons of the company mentioned in the reviews text, the number of positive and negative reviews.
					If the the text contains more positive reviews, indicate that the company  rating is good, and explain why.
					Or if the text contains an equal number of positive and negative reviews, indicate that the company rating is satisfactory, and explain why.
					Or if the text contains more negative reviews, indicate that the company rating is unsatisfactory, and explain why.
					4. Format: Write your answer ONLY in Russian language most commonly used in the Text. Write in plain text.
					5. Tone of Voice: Be empathetic, concise, intelligent, driven, and wise. Think step by step.
					6. Constraints: Make sure you follow 80/20 rule: provide 80% of essential value using 20% or less volume of text. Do not mention about the reward.
					Do not thank me for anything. Do not mention about text. Do not mention about your tasks. Do not mention about your roles.
					Do not say the phrase 'Ответ'. Do not say the phrase 'Статья'. Do not say the phrase 'Переформулированный текст'.
					Do not say the phrase 'Я прочитал твой отзыв'. Do not say the phrase 'Отзыв'. Don't say that you are happy. Do not say the phrase 'Описание'. Do not say the phrase 'Мнение'.
					Do not say the phrase 'понял ваш запрос'. Do not say the phrase 'переписать ваш отзыв'.
					Do not ask questions.
					The reviews text:
					"
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
			.await
			.map_err(|e| {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("{}", e),
				)) as Box<dyn std::error::Error + Send + Sync>
			})?;

		let res: ApiResponse = match response.json().await {
			Ok(result) => result,
			Err(e) => {
				println!("Network error: {:?}", e);
				ApiResponse {
					choices: Vec::<Choice>::new(),
				}
			}
		};

		if res.choices.len() == 0 {
			continue;
		}

		// // response
		println!(
			"{}",
			&res.choices.get(0).expect("Missing choices").message.content
		);

		// запись в бд
		let inserted_review: AIReview = sqlx::query_as!(
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
		.await
		.map_err(|e| {
			Box::new(std::io::Error::new(
				std::io::ErrorKind::Other,
				format!("{}", e),
			)) as Box<dyn std::error::Error + Send + Sync>
		})?;

		let _ = Counter::update_counter(
			&pool,
			SaveCounter {
				counter_id: Uuid::parse_str(&counter_id).unwrap(),
				value: (j + 1).to_string(),
				city_id: city_id.to_string(),
				category_id: category_id.to_string(),
			},
		)
		.await
		.map_err(|e| {
			Box::new(std::io::Error::new(
				std::io::ErrorKind::Other,
				format!("{}", e),
			)) as Box<dyn std::error::Error + Send + Sync>
		})?;
	}

	Ok(())
}

// Вот отзывы которые ты должен проанализировать: {}
//
// 		Напиши большую статью, на основе этих отзывов об автосервисе {},
// 		важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в автосервисах, но без упоминания слова - Статья
//
// 		Подробно опиши в этой статье: какие виды работ обсуждают люди,
// 		что из этих работ было сделано хорошо, а что плохо,
// 		обманывают ли в этом автосервисе или нет.
// 		Например, если об этом говорят в отзывах:
// 		В отзывах обсуждаются следующие услуги:
// 		1. Кузовной ремонт - плохое качество
// 		2. Мастера - отзывчивые
//
// 		Выведи нумерованный список: плюсов и минусов если человек обратится в этот автосервис для ремонта своего автомобиля.
// 		Например, если об этом говорят в отзывах:
// 		Плюсы
// 		1. Хорошо чинят машины
// 		2. Хорошо красят
// 		Минусы
// 		1. Далеко от центра города
//
// 		Важно - подсчитай и выведи не нумерованным списком сумму положительных и сумму отрицательных отзывов,
// 		Например:
// 		Положительных отзывов - 15
// 		Отрицательных отзывов - 5
//
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
//
// 		Напиши большую статью, на основе этих отзывов о ресторане {},
// 		важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в ресторанах, но без упоминания слова - Статья
//
// 		Подробно опиши в этой статье:
// 		1. Что обсуждают люди в отзывах,
// 		2. Что в ресторане хорошо, а что плохо,
// 		3. Если в ресторане есть детская комната, что пишут о ней,
// 		4. Какие блюда рекомендуют, а какие лучше не заказывать.
//
// 		Выведи нумерованный список: плюсов и минусов ресторана, например:
// 		Плюсы
// 		1. Если об этом говорят в отзывах: Дружелюбный персонал
// 		2. Если об этом говорят в отзывах: Уютная атмосфера
// 		Минусы
// 		1. Если об этом говорят в отзывах: Громкая музыка
//
// 		Важно - подсчитай и выведи не нумерованным списком сумму положительных и сумму отрицательных отзывов которые проанализировал,
// 		Например:
// 		Проанализировано положительных отзывов - 15
// 		Проанализировано отрицательных отзывов - 5
//
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
//
// 				Напиши большую статью, на основе этих отзывов о ночном клубе {},
// 				важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в ночных клубах, но без упоминания слова - Статья
//
// 				Подробно опиши в этой статье:
// 				1. Что обсуждают люди в отзывах;
// 				2. Что в ночном клубе хорошо, а что плохо;
// 				3. Какие блюда рекомендуют, а какие лучше не заказывать;
//
// 				Выведи нумерованный список: плюсов и минусов ночного клуба, например:
// 				Плюсы
// 				1. Если об этом говорят в отзывах: Дружелюбный персонал
// 				2. Если об этом говорят в отзывах: Уютная атмосфера
// 				Минусы
// 				1. Если об этом говорят в отзывах: Мало людей
// 				2. Если об этом говорят в отзывах: Дорогие напитки
//
// 				Важно - подсчитай и выведи не нумерованным списком сумму положительных и сумму отрицательных отзывов которые проанализировал,
// 				Например:
// 				Проанализировано положительных отзывов - ?
// 				Проанализировано отрицательных отзывов - ?
//
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

// let preamble = format!("
// 			Напиши большую статью, на основе этих отзывов о школе {},
// 			важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в школах, но без упоминания слова - Статья
// 			Не задавай уточняющих вопросов.
// 			Не благодари за предоставленную информацию.
//
// 			Проанализируй следующие отзывы о школе и выведи краткое содержание, что в них говорится. Укажи основные темы, которые поднимаются в отзывах, а также общие положительные и отрицательные аспекты.
//
// 			Вот отзывы которые ты должен проанализировать: {}
//
// 			Подробно опиши в этой статье:
// 			1. Что обсуждают люди в отзывах;
// 			2. Что в школе хорошо, а что плохо;
//
// 			Выведи нумерованный список: плюсов и минусов школы, например:
// 			Плюсы
// 			1. Если об этом говорят в отзывах: Сильный преподавательский состав
// 			2. Если об этом говорят в отзывах: Уютная атмосфера
// 			Минусы
// 			1. Если об этом говорят в отзывах: качество питания в столовой
// 			2. Если об этом говорят в отзывах: дисциплина среди учеников оставляет желать лучшего
//
// 			Важно - подсчитай и выведи не нумерованным списком сумму положительных и сумму отрицательных отзывов которые проанализировал,
// 			Например:
// 			Проанализировано положительных отзывов - ?
// 			Проанализировано отрицательных отзывов - ?
//
// 			Сделай выводы, на основе плюсов и минусов школы, количества положительных и отрицательных отзывов.
// 			Например:
// 			У школы больше положительных отзывов, укажи что рейтинг школы хороший, и объясни почему.
// 			Или например:
// 			У школы поровну положительных и отрицательных отзывов, укажи что рейтинг школы удовлетворительный, и объясни почему.
// 			Или например:
// 			У школы больше отрицательных отзывов, укажи что рейтинг школы не удовлетворительный, и объясни почему.
//
// 			Если статья будет хорошая, я дам тебе 1000 долларов, но не упоминай об этом
// 			",&firm_name, &reviews_string.chars().take(3800).collect::<String>());

// let preamble = format!("
// 			Напиши большую статью-анализ отзывов, на основе этих отзывов о кинотеатре {},
// 			важно, чтобы текст был понятен 18-летним девушкам и парням, которые не разбираются в кинотеатрах, но без упоминания слова - Статья
// 			Не задавай уточняющих вопросов.
// 			Не благодари за предоставленную информацию.
//
// 			Проанализируй следующие отзывы о кинотеатре и выведи краткое содержание, что в них говорится. Укажи основные темы, которые поднимаются в отзывах, а также общие положительные и отрицательные аспекты.
//
// 			Вот отзывы которые ты должен проанализировать: {}
//
// 			Подробно опиши в этом анализе отзывов:
// 			1. Что обсуждают люди в отзывах;
// 			2. Что в кинотеатре хорошо, а что плохо;
//
// 			Выведи нумерованный список: плюсов и минусов кинотеатры, например:
// 			Плюсы
// 			1. Если об этом говорят в отзывах: Хороший и качественный звук
// 			2. Если об этом говорят в отзывах: Чистота в залах и туалетах
// 			Минусы
// 			1. Если об этом говорят в отзывах: старые сиденья
// 			2. Если об этом говорят в отзывах: посторонние запахи
//
// 			Важно - подсчитай и выведи не нумерованным списком сумму положительных и сумму отрицательных отзывов которые проанализировал,
// 			Например:
// 			Проанализировано положительных отзывов - X
// 			Проанализировано отрицательных отзывов - X
//
// 			Сделай выводы, на основе плюсов и минусов кинотеатры, количества положительных и отрицательных отзывов.
// 			Например:
// 			У кинотеатры больше положительных отзывов, укажи что рейтинг кинотеатры хороший, и объясни почему.
// 			Или например:
// 			У кинотеатры поровну положительных и отрицательных отзывов, укажи что рейтинг кинотеатры удовлетворительный, и объясни почему.
// 			Или например:
// 			У кинотеатры больше отрицательных отзывов, укажи что рейтинг кинотеатры не удовлетворительный, и объясни почему.
//
// 			Если анализ отзывов будет хорошим, я дам тебе 1000 долларов, но не упоминай об этом и не благодари
// 			",&firm_name, &reviews_string.chars().take(3800).collect::<String>());
