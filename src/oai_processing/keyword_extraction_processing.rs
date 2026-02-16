use crate::models::rabbitmq::AIProcessingTask;
use crate::oai_processing::oai_title_processing::process_title_with_qwen_cli;
use crate::services::rabbitmq_producer::RabbitMQProducer;
use serde_json::Value;
use sqlx::PgPool;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Структура для хранения данных о replacement из БД
#[derive(Debug, sqlx::FromRow)]
pub struct ReplacementData {
	pub replacement_id: uuid::Uuid,
	pub old_ad_id: uuid::Uuid,
	pub feed_id: uuid::Uuid,
	pub status: String,
	pub old_ad_title: Option<String>,
	pub old_ad_description: Option<String>,
}

pub async fn process_keyword_extraction_with_qwen_cli(
	pool: PgPool,
	task: &AIProcessingTask,
	producer: Arc<Mutex<Option<RabbitMQProducer>>>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
	println!("🔍 Processing batch keyword extraction task: {}", task.task_id);

	// Extract feed_id and batch info from task parameters
	let feed_id = task
		.request_data
		.parameters
		.get("feed_id")
		.and_then(|v| v.as_str())
		.and_then(|s| uuid::Uuid::parse_str(s).ok())
		.or_else(|| {
			task.request_data
				.parameters
				.get("feed_id")
				.and_then(|v| serde_json::from_value::<uuid::Uuid>(v.clone()).ok())
		});

	let batch_id = task
		.request_data
		.parameters
		.get("batch_id")
		.and_then(|v| v.as_str())
		.unwrap_or("unknown_batch");

	let total_replacements = task
		.request_data
		.parameters
		.get("total_replacements")
		.and_then(|v| v.as_i64())
		.unwrap_or(0) as i32;

	let user_id = task.request_data.user_id;

	// Получаем feed_id как строку и парсим
	let feed_id = match feed_id {
		Some(id) => id,
		None => {
			return Err(Box::new(std::io::Error::new(
				std::io::ErrorKind::InvalidInput,
				"Missing feed_id in task parameters",
			)));
		}
	};

	println!(
		"📊 Batch keyword extraction: feed_id={}, batch_id={}, total_replacements={}",
		feed_id, batch_id, total_replacements
	);

	// Fetch all replacements from avito_ad_replacements table for this feed
	let replacements = fetch_replacements_from_db(&pool, feed_id).await?;
	let actual_total = replacements.len();

	println!(
		"📊 Fetched {} replacements from database for feed {}",
		actual_total, feed_id
	);

	if actual_total == 0 {
		return Err(Box::new(std::io::Error::new(
			std::io::ErrorKind::NotFound,
			format!("No replacements found for feed {}", feed_id),
		)));
	}

	// Process each replacement and send results via RabbitMQ
	let mut processed_count = 0;

	for (index, replacement) in replacements.iter().enumerate() {
		println!(
			"🔍 Processing replacement {}/{}: {} (ad: {})",
			index + 1,
			actual_total,
			replacement.replacement_id,
			replacement.old_ad_id
		);

		let title = replacement.old_ad_title.as_deref().unwrap_or("");
		let description = replacement.old_ad_description.as_deref().unwrap_or("");

		// Skip if both title and description are empty
		if title.is_empty() && description.is_empty() {
			println!(
				"⚠️ Skipping replacement {} - no title or description",
				replacement.replacement_id
			);
			processed_count += 1;
			
			// Send progress update for skipped item
			if let Some(ref prod) = *producer.lock().await {
				let progress = ((index + 1) as f64 / actual_total as f64 * 100.0) as i32;
				let result_data = serde_json::json!({
					"batch_id": batch_id,
					"feed_id": feed_id.to_string(),
					"replacement_id": replacement.replacement_id.to_string(),
					"old_ad_id": replacement.old_ad_id.to_string(),
					"keywords": "",
					"progress": progress,
					"total": actual_total,
					"skipped": true
				});
				
				let _ = prod.send_result(
					replacement.replacement_id,
					user_id,
					Some(feed_id),
					"completed",
					Some(result_data),
					None,
				).await;
			}
			continue;
		}

		// Create a prompt for extracting keywords
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
			6. Ответ должен содержать только список ключевых слов (не более 3-4 слов или 1-2 словосочетания), без дополнительных комментариев

			Пример:
			Если заголовок: "Помпа КАМАЗ с доставкой №344011"
			То результат: "помпа КАМАЗ"

			Ответ:
			"#,
			title, description
		);

		// Create a temporary task for LLM call
		let temp_task = AIProcessingTask {
			task_id: uuid::Uuid::new_v4(),
			request_data: crate::models::rabbitmq::AIRequestData {
				request_id: replacement.replacement_id,
				user_id,
				processing_type: "title".to_string(),
				parameters: serde_json::json!({
					"input_text": prompt,
					"title": title,
					"description": description,
				}),
			},
			created_at: task.created_at.clone(),
		};

		// Process with LLM
		let result = process_title_with_qwen_cli(pool.clone(), &temp_task).await?;
		let keywords = clean_keyword_output(&result);

		println!(
			"✅ Keywords for replacement {}: {}",
			replacement.replacement_id, keywords
		);

		// Calculate progress
		let progress = ((index + 1) as f64 / actual_total as f64 * 100.0) as i32;

		// Send result via RabbitMQ to A-back
		if let Some(ref prod) = *producer.lock().await {
			let result_data = serde_json::json!({
				"batch_id": batch_id,
				"feed_id": feed_id.to_string(),
				"replacement_id": replacement.replacement_id.to_string(),
				"old_ad_id": replacement.old_ad_id.to_string(),
				"keywords": keywords,
				"progress": progress,
				"total": actual_total,
				"processing_type": "keyword_extraction"
			});

			if let Err(e) = prod
				.send_result(
					replacement.replacement_id,
					user_id,
					Some(feed_id),
					"completed",
					Some(result_data),
					None,
				)
				.await
			{
				eprintln!("❌ Failed to send keyword extraction result: {}", e);
			} else {
				println!(
					"✅ Sent keyword extraction result for replacement {} (progress: {}/{})",
					replacement.replacement_id, index + 1, actual_total
				);
			}
		}

		processed_count += 1;
	}

	println!(
		"✅ Batch keyword extraction completed: {}/{} replacements processed",
		processed_count, actual_total
	);

	// Send final completion message
	if let Some(ref prod) = *producer.lock().await {
		let completion_data = serde_json::json!({
			"batch_id": batch_id,
			"feed_id": feed_id.to_string(),
			"total_replacements": actual_total,
			"processed": processed_count,
			"progress": 100,
			"all_completed": true,
			"processing_type": "keyword_extraction"
		});

		if let Err(e) = prod
			.send_result(
				task.task_id,
				user_id,
				Some(feed_id),
				"all_completed",
				Some(completion_data),
				None,
			)
			.await
		{
			eprintln!("❌ Failed to send batch completion message: {}", e);
		} else {
			println!("✅ Sent batch completion message for feed {}", feed_id);
		}
	}

	Ok(format!("Processed {} replacements", processed_count))
}

async fn fetch_replacements_from_db(
	pool: &PgPool,
	feed_id: uuid::Uuid,
) -> Result<Vec<ReplacementData>, Box<dyn Error + Send + Sync>> {
	// Query avito_ad_replacements and join with avito_ad_fields to get title and description
	let replacements = sqlx::query_as!(
		ReplacementData,
		r#"
        SELECT 
            r.replacement_id,
            r.old_ad_id,
            r.feed_id,
            r.status,
            title_field.value as old_ad_title,
            desc_field.value as old_ad_description
        FROM avito_ad_replacements r
        LEFT JOIN LATERAL (
            SELECT afv.value
            FROM avito_ad_fields af
            JOIN avito_ad_field_values afv ON af.field_id = afv.field_id
            WHERE af.ad_id = r.old_ad_id AND af.tag = 'Title'
            LIMIT 1
        ) title_field ON true
        LEFT JOIN LATERAL (
            SELECT afv.value
            FROM avito_ad_fields af
            JOIN avito_ad_field_values afv ON af.field_id = afv.field_id
            WHERE af.ad_id = r.old_ad_id AND af.tag = 'Description'
            LIMIT 1
        ) desc_field ON true
        WHERE r.feed_id = $1
        "#,
		feed_id
	)
	.fetch_all(pool)
	.await?;

	Ok(replacements)
}

fn clean_keyword_output(output: &str) -> String {
	// Remove any extra text that might be included in the response
	let lower_output = output.to_lowercase();

	// Look for keywords after common indicators
	let mut cleaned = output.trim().to_string();

	if let Some(pos) = lower_output.find("ответ:") {
		cleaned = output[pos + 6..].trim().to_string();
	} else if let Some(pos) = lower_output.find("результат:") {
		cleaned = output[pos + 10..].trim().to_string();
	} else if let Some(pos) = lower_output.find("keywords:") {
		cleaned = output[pos + 9..].trim().to_string();
	} else if let Some(pos) = lower_output.find("ключевые слова:") {
		cleaned = output[pos + 15..].trim().to_string();
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
