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
	message: Message,  // Ollama API returns message with content
	done: bool,
	#[serde(default)]
	done_reason: Option<String>,
	#[serde(default)]
	total_duration: Option<u64>,
	#[serde(default)]
	load_duration: Option<u64>,
	#[serde(default)]
	prompt_eval_count: Option<u32>,
	#[serde(default)]
	prompt_eval_duration: Option<u64>,
	#[serde(default)]
	eval_count: Option<u32>,
	#[serde(default)]
	eval_duration: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Message {
	role: String,
	content: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct OpenAIResponse {
	id: String,
	object: String,
	created: u64,
	model: String,
	choices: Vec<Choice>,
	usage: Option<Usage>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Choice {
	index: u32,
	message: Message,
	finish_reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Usage {
	prompt_tokens: u32,
	completion_tokens: u32,
	total_tokens: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AiDescriptionProcessingMessage {
	pub task_id: Uuid,
	pub user_id: Uuid,
	pub description: String,
	pub category: String,
	pub created_ts: chrono::DateTime<chrono::Utc>,
}

#[allow(unreachable_code)]
pub async fn oai_description_processing(
	pool: Pool<Postgres>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
	loop {
		let mut needs_to_restart = true;
		if needs_to_restart {
			// This function is now a placeholder since the actual processing happens
			// in the RabbitMQ consumer with a specific task
			// The actual processing logic is in the process_description_with_ai function
			return Ok(());
		}
	}

	Ok(())
}

pub async fn process_description_with_ai(
	pool: Pool<Postgres>,
	task: &AIProcessingTask,
) -> Result<String, Box<dyn Error + Send + Sync>> {
	// Extract the description and category from the task parameters
	let description = task
		.request_data
		.parameters
		.get("description")
		.and_then(|v| v.as_str())
		.ok_or("Description not found in task parameters")?
		.to_string();

	let category = task
		.request_data
		.parameters
		.get("category")
		.and_then(|v| v.as_str())
		.unwrap_or("General"); // Default to "General" if category is not provided

	println!("Processing description: {}, category: {}", description, category);

	// Get the AI model parameters
	let url = env::var("OPENAI_API_BASE").expect("OPENAI_API_BASE not set");
	let open_ai_token = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

	// Create the AI request with the system prompt for beautifying descriptions for Avito ads
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
                You are an expert at creating attractive and effective descriptions for Avito advertisements.
                Your task is to beautify and optimize a description to make it more appealing to potential customers on Avito.
                Consider the category when crafting the description to ensure it's appropriate and compelling for that specific market segment.
                Make the description engaging, clear, and compelling while maintaining the original meaning.
                Focus on benefits, quality, and value proposition that resonate with the target audience in the given category.
                Keep the description concise but impactful.
                Write in Russian language.
                Respond ONLY with the beautified description, nothing else.
                Do not include any explanations, analysis, or additional text.
                "
			},
			{
				"role": "user",
				"content": format!("Beautify this description for an Avito ad in the category '{}': {}. Respond ONLY with the beautified description, nothing else.", category, description)
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

	// Try to parse the response as JSON - handle both Ollama and OpenAI formats
	let beautified_description = if let Ok(ollama_response) = serde_json::from_str::<OllamaResponse>(&response_text) {
		// Handle Ollama response format - extract content from the message field
		ollama_response.message.content
	} else if let Ok(openai_response) = serde_json::from_str::<OpenAIResponse>(&response_text) {
		// Handle OpenAI response format
	openai_response.choices
			.first()
			.map(|choice| choice.message.content.clone())
			.ok_or_else(|| {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					"No choices found in OpenAI response",
				)) as Box<dyn std::error::Error + Send + Sync>
			})?
	} else {
		// If both parsing attempts fail, return an error
		return Err(Box::new(std::io::Error::new(
			std::io::ErrorKind::Other,
			"Failed to parse response as either Ollama or OpenAI format",
		)) as Box<dyn std::error::Error + Send + Sync>);
	};

	// Print the result to terminal
	println!("Original description: {}", description);
	println!("Beautified description: {}", beautified_description);

	Ok(beautified_description)
}