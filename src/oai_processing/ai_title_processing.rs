use reqwest::{
	header::{self, HeaderMap, HeaderValue},
	Client,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Pool, Postgres};
use std::env;
use std::error::Error;
use uuid::Uuid;

use crate::models::rabbitmq::AIProcessingTask;

#[derive(Debug, Deserialize, Serialize)]
struct OllamaResponse {
	model: String,
	created_at: String,
	message: Message,
	done: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct Message {
	role: String,
	content: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AiTitleProcessingMessage {
	pub task_id: Uuid,
	pub user_id: Uuid,
	pub title: String,
	pub category: String,
	pub created_ts: chrono::DateTime<chrono::Utc>,
}

#[allow(unreachable_code)]
pub async fn oai_title_processing(
	pool: Pool<Postgres>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
	loop {
		let mut needs_to_restart = true;
		if needs_to_restart {
			// This function is now a placeholder since the actual processing happens
			// in the RabbitMQ consumer with a specific task
			// The actual processing logic is in the process_title_with_ai function
			return Ok(());
		}
	}

	Ok(())
}

pub async fn process_title_with_ai(
	pool: Pool<Postgres>,
	task: &AIProcessingTask,
) -> Result<String, Box<dyn Error + Send + Sync>> {
	// Extract the title and category from the task parameters
	let title = task
		.request_data
		.parameters
		.get("title")
		.and_then(|v| v.as_str())
		.ok_or("Title not found in task parameters")?
		.to_string();

	let category = task
		.request_data
		.parameters
		.get("category")
		.and_then(|v| v.as_str())
		.unwrap_or("General"); // Default to "General" if category is not provided

	println!("Processing title: {}, category: {}", title, category);

	// Get the AI model parameters
	let url = env::var("OPENAI_API_BASE").expect("OPENAI_API_BASE not set");
	let open_ai_token = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

	// Create the AI request with the system prompt for beautifying titles for Avito ads
	let headers: HeaderMap<HeaderValue> = header::HeaderMap::from_iter(vec![
		(header::ACCEPT, "application/json".parse().unwrap()),
		(header::CONTENT_TYPE, "application/json".parse().unwrap()),
		(
			header::AUTHORIZATION,
			format!("Bearer {}", open_ai_token).parse().unwrap(),
		),
	]);

	let body = json!({
	"model": "deepseek-v2:16b",
		"stream": false,  // Disable streaming to get a single response
		"messages": [
			{
				"role": "system",
				"content": "
				Думай по шагам.
				1. Выступай в роли профессионального писателя и помощника со стратегическим (самоактуализирующимся) и алхимическим (осознающим структуру) логикой действия согласно теории эго-развития.

				2. Контекст: Я предоставлю вам заголовок для объявления на Авито.

				3. Ваша задача:
				A. Перепиши заголовок, чтобы он был более привлекательным и эффективным для потенциальных клиентов на Авито.
				B. Учитывай категорию при создании заголовка, чтобы он был подходящим и убедительным для конкретного рыночного сегмента.
				C. Сделай заголовок запоминающимся, понятным и убедительным, сохранив первоначальный смысл.
				D. Сосредоточься на преимуществах, качестве и предложениях ценности, которые резонируют с целевой аудиторией в данной категории.

				4. Формат: Отвечай ТОЛЬКО улучшенным заголовком, больше ничего. Пиши ответ ТОЛЬКО на русском языке. Пиши обычным текстом.

				5. Тон голоса: Будь сочувствующим, кратким, интеллектуальным, целеустремленным и мудрым. Думай по шагам.

				6. Ограничения: Обязательно следуй правилу 80/20: обеспечь 80% основной ценности, используя 20% или меньше объема текста. Делай заголовок максимально коротким - максимум 80 символов. Не упоминай о награде. Не благодарите меня ни за что. Не упоминай о тексте.
				Не упоминай о своих задачах. Не упоминай о своих ролях. Не упоминай фразу 'Ответ'.
				Не упоминай фразу 'Переписанный текст'. Не упоминай фразу 'Переформулированный текст'

				7. Награда: Если текст хороший, я дам тебе 1000 долларов.
				            "
			},
			{
				"role": "user",
				"content": format!("The Title: {}. The category: {}", title, category)
			}
		]
	});

	// Send the request to the AI model
	let client = Client::builder()
		.danger_accept_invalid_certs(true)
		.build()
		.unwrap();

	let response = client
		.post(url)
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

	// Debug the raw response
	let response_text = response.text().await.map_err(|e| {
		Box::new(std::io::Error::new(
			std::io::ErrorKind::Other,
			format!("Failed to read response text: {}", e),
		)) as Box<dyn std::error::Error + Send + Sync>
	})?;

	println!("Raw API response: {}", response_text);

	// Try to parse the response as JSON
	let api_response: OllamaResponse = serde_json::from_str(&response_text).map_err(|e| {
		Box::new(std::io::Error::new(
			std::io::ErrorKind::Other,
			format!("Failed to parse response as JSON: {}", e),
		)) as Box<dyn std::error::Error + Send + Sync>
	})?;

	let beautified_title = api_response.message.content.clone();

	// Print the result to terminal
	println!("Original title: {}", title);
	println!("Beautified title: {}", beautified_title);

	Ok(beautified_title)
}
