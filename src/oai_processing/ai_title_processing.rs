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
                You are an expert at creating attractive and effective titles for Avito advertisements.
                Your task is to beautify and optimize a title to make it more appealing to potential customers on Avito.
                Consider the category when crafting the title to ensure it's appropriate and compelling for that specific market segment.
                Make the title catchy, clear, and compelling while maintaining the original meaning.
                Focus on benefits, quality, and value proposition that resonate with the target audience in the given category.
                Keep the title concise but impactful.
                Write in Russian language.
                Respond ONLY with the beautified title, nothing else.
                Do not include any explanations, analysis, or additional text.
                "
			},
			{
				"role": "user",
				"content": format!("Beautify this title for an Avito ad in the category '{}': {}. Respond ONLY with the beautified title, nothing else.", category, title)
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
