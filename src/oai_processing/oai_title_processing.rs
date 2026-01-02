use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::error::Error;
use std::process::Command;
use uuid::Uuid;

use crate::models::rabbitmq::AIProcessingTask;

#[derive(Debug, Deserialize, Serialize)]
pub struct QwenCliProcessingMessage {
	pub task_id: Uuid,
	pub user_id: Uuid,
	pub input_text: String,
	pub category: String,
	pub created_ts: chrono::DateTime<chrono::Utc>,
}

pub async fn process_title_with_qwen_cli(
	pool: Pool<Postgres>,
	task: &AIProcessingTask,
) -> Result<String, Box<dyn Error + Send + Sync>> {
	// Extract the input text and category from the task parameters
	let input_text = task
		.request_data
		.parameters
		.get("input_text")
		.and_then(|v| v.as_str())
		.or_else(|| {
			task.request_data
				.parameters
				.get("title")
				.and_then(|v| v.as_str())
		})
		.ok_or("Title not found in task parameters. Expected 'input_text' or 'title' field.")?
		.to_string();

	let category = task
		.request_data
		.parameters
		.get("category")
		.and_then(|v| v.as_str())
		.unwrap_or("General"); // Default to "General" if category is not provided

	println!(
		"Processing title with qwen-cli: {}, category: {}",
		input_text, category
	);

	// Prepare a contextual prompt for the qwen-cli command
	// Simple and direct prompt to ensure only the requested content is returned
	let system_prompt = "Ты - инструмент генерации заголовков и профессиональный маркетолог. Отвечай ТОЛЬКО текстом, который нужно скопировать и вставить. НЕ добавляй никаких поясняющих слов, комментариев, мыслей или системных сообщений. НЕ говори 'Ответ:', 'Результат:' или что-либо подобное. Просто предоставь запрашиваемый контент.";

	let user_prompt = format!("Категория: {}. Задача перефразировать и улучшить заголовок для объявления на доске объявлений авито: {}", category, input_text);

	// Combine system and user prompts
	let contextual_prompt = format!(
		"{}\n\nПользовательский запрос: {}",
		system_prompt, user_prompt
	);

	// Prepare arguments for the qwen-cli command
	let args = vec![
		"-p".to_string(), // Use prompt flag for non-interactive mode
		contextual_prompt,
	];

	// Execute the qwen-cli command
	let output = Command::new("npx")
		.arg("@qwen-code/qwen-code")
		.args(&args)
		.output()
		.map_err(|e| {
			eprintln!("Failed to execute qwen-cli: {}", e);
			Box::new(std::io::Error::new(
				std::io::ErrorKind::Other,
				format!("Failed to execute qwen-cli: {}", e),
			)) as Box<dyn std::error::Error + Send + Sync>
		})?;

	if !output.status.success() {
		let stderr =
			String::from_utf8(output.stderr).unwrap_or_else(|_| "Unknown error".to_string());
		eprintln!("qwen-cli command failed: {}", stderr);
		return Err(Box::new(std::io::Error::new(
			std::io::ErrorKind::Other,
			format!("qwen-cli command failed: {}", stderr),
		)));
	}

	// Parse the output from qwen-cli
	let result = String::from_utf8(output.stdout).map_err(|e| {
		eprintln!("Failed to parse qwen-cli output: {}", e);
		Box::new(std::io::Error::new(
			std::io::ErrorKind::Other,
			format!("Failed to parse qwen-cli output: {}", e),
		)) as Box<dyn std::error::Error + Send + Sync>
	})?;

	// Print the result to terminal
	println!("Input title: {}", input_text);
	println!("Qwen-cli result: {}", result);

	Ok(result.trim().to_string())
}

pub async fn oai_title_processing(
	pool: Pool<Postgres>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
	loop {
		let mut needs_to_restart = true;
		if needs_to_restart {
			// This function is now a placeholder since the actual processing happens
			// in the RabbitMQ consumer with a specific task
			// The actual processing logic is in the process_title_with_qwen_cli function
			return Ok(());
		}
	}

	Ok(())
}
