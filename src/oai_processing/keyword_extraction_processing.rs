use crate::models::rabbitmq::AIProcessingTask;
use crate::oai_processing::oai_title_processing::process_title_with_qwen_cli;
use serde_json::Value;
use sqlx::PgPool;
use std::error::Error;

pub async fn process_keyword_extraction_with_qwen_cli(
	pool: PgPool,
	task: &AIProcessingTask,
) -> Result<String, Box<dyn Error + Send + Sync>> {
	println!("🔍 Processing keyword extraction task: {}", task.task_id);

	// Extract the input text from the task parameters
	let input_text = if let Some(input_value) = task.request_data.parameters.get("input_text") {
		input_value.as_str().unwrap_or("")
	} else {
		return Err(Box::new(std::io::Error::new(
			std::io::ErrorKind::InvalidInput,
			"Missing input_text in task parameters",
		)));
	};

	let title = if let Some(title_value) = task.request_data.parameters.get("title") {
		title_value.as_str().unwrap_or("")
	} else {
		return Err(Box::new(std::io::Error::new(
			std::io::ErrorKind::InvalidInput,
			"Missing title in task parameters",
		)));
	};

	let description = if let Some(desc_value) = task.request_data.parameters.get("description") {
		desc_value.as_str().unwrap_or("")
	} else {
		""
	};

	// Create a prompt for extracting keywords from the title and description
	let prompt = format!(
		r#"
		Ты - опытный SEO-специалист и маркетолог. Твоя задача - извлечь ключевые слова из заголовка и описания объявления, убрав мусорные слова, которые не относятся к теме товара или услуги.

		Исходный заголовок: "{}"
		Описание: "{}"

		Требования:
		1. Извлечь только ключевые слова для поиска на авито, которые описывают суть товара/услуги, по возможности не более 2-3 слов
		2. Удалить слова, которые не относятся к теме (например: "ЗВОНИТЕ", "ГАРАНТИЯ", "ДОСТАВКА", "ОПЛАТА", "СКИДКА", "НОВЫЙ", "Б/У", и т.д.)
		3. Удалить любые номера или артикулы
		4. Оставить только слова, которые описывают сам продукт/услугу
		5. Вернуть результат в виде списка ключевых слов, разделенных запятыми
		6. Ответ должен содержать только список ключевых слов, без дополнительных комментариев

		Пример:
		Если заголовок: "Помпа КАМАЗ с доставкой №344011"
		То результат: "помпа КАМАЗ"

		Ответ:
		"#,
		title, description
	);

	// Create a temporary task with our custom prompt for keyword extraction
	let temp_task = AIProcessingTask {
		task_id: task.task_id, // Use the original task ID
		request_data: crate::models::rabbitmq::AIRequestData {
			request_id: task.request_data.request_id,
			user_id: task.request_data.user_id,
			processing_type: "title".to_string(), // Use title processing type for the LLM call
			parameters: serde_json::json!({
				"input_text": prompt, // Use our custom prompt
				"title": title, // Keep original title for context
				"description": description, // Keep original description for context
			}),
		},
		created_at: task.created_at.clone(),
	};

	// Use the existing title processing function with our custom task
	let result = process_title_with_qwen_cli(pool, &temp_task).await?;

	// Clean up the result to ensure it's just keywords
	let cleaned_result = clean_keyword_output(&result);

	println!("✅ Keyword extraction completed for task: {}", task.task_id);
	println!("Keywords extracted: {}", cleaned_result);

	Ok(cleaned_result)
}

fn clean_keyword_output(output: &str) -> String {
	// Remove any extra text that might be included in the response
	// Use a simple approach without regex since it's not available
	let lower_output = output.to_lowercase();

	// Look for keywords after common indicators
	let mut cleaned = output.trim().to_string();

	if let Some(pos) = lower_output.find("ответ:") {
		cleaned = output[pos + 6..].trim().to_string(); // Skip "ответ:" and any following spaces
	} else if let Some(pos) = lower_output.find("результат:") {
		cleaned = output[pos + 10..].trim().to_string(); // Skip "результат:" and any following spaces
	} else if let Some(pos) = lower_output.find("keywords:") {
		cleaned = output[pos + 9..].trim().to_string(); // Skip "keywords:" and any following spaces
	} else if let Some(pos) = lower_output.find("ключевые слова:") {
		cleaned = output[pos + 15..].trim().to_string(); // Skip "ключевые слова:" and any following spaces
	}

	// Remove quotes if present
	let cleaned = cleaned.trim_matches('"').trim_matches('\'');

	// Normalize spaces and commas
	let cleaned = cleaned.replace("\n", ", ").replace("\r", ", ");
	let cleaned = cleaned
		.replace("  ", " ")
		.replace(", ,", ",")
		.replace(",,", ",");

	cleaned.trim().to_string()
}
