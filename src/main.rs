mod api;
mod config;
mod models;
mod oai_processing;
mod processing;
mod services;
mod utils;

use crate::models::rabbitmq::{AIProcessingResult, AIProcessingTask, AIRequestData};
use crate::oai_processing::oai_description_processing::oai_description_processing;
use crate::oai_processing::oai_title_processing::oai_title_processing;
use crate::services::rabbitmq_consumer::RabbitMQConsumer;
use crate::services::rabbitmq_producer::RabbitMQProducer;
use config::Config;
use dotenv::dotenv;
use oai_processing::{
	oai_pages_processing, oai_reviews_processing, oai_reviews_rewrite_processing,
};
use processing::{
	images_processing, pages_sitemap_processing, reviews_count_processing, sitemap_processing,
	title_processing, urls_processing,
};
use reqwest::{
	header::{self, HeaderMap, HeaderValue},
	Body, Client,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

#[feature(proc_macro_byte_character)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
	dotenv().ok();

	if std::env::var_os("RUST_LOG").is_none() {
		std::env::set_var("RUST_LOG", "actix_web=info");
	}
	env_logger::init();

	let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "direct".to_string());

	match run_mode.as_str() {
		"direct" => {
			// Original direct execution mode
			let config = Config::init();
			println!("Starting command bot in direct mode...");

			let pool = match PgPoolOptions::new()
				.max_connections(10)
				.connect(&config.database_url)
				.await
			{
				Ok(pool) => {
					println!("✅ Connection to the database is successful!");
					pool
				}
				Err(err) => {
					println!("🔥 Failed to connect to the database: {:?}", err);
					std::process::exit(1);
				}
			};

			let processing_type = env::var("PROCESSING_TYPE").expect("PROCESSING_TYPE not set");
			println!("PROCESSING_TYPE: {}", &processing_type);

			// Handle each processing type with appropriate error conversion
			match processing_type.as_str() {
				"title" => {
					oai_title_processing(pool.clone()).await.map_err(|e| {
						Box::new(std::io::Error::new(
							std::io::ErrorKind::Other,
							format!("{}", e),
						)) as Box<dyn std::error::Error + Send + Sync>
					})?;
				}
				"description" => {
					match oai_description_processing(pool.clone()).await {
						Ok(_) => {}
						Err(e) => {
							// Convert the Send + Sync error back to a standard error for the direct mode
							return Err(Box::new(std::io::Error::new(
								std::io::ErrorKind::Other,
								format!("{}", e),
							)) as Box<dyn std::error::Error + Send + Sync>);
						}
					}
				}
				"images" => {
					images_processing().await.map_err(|e| {
						Box::new(std::io::Error::new(
							std::io::ErrorKind::Other,
							format!("{}", e),
						)) as Box<dyn std::error::Error + Send + Sync>
					})?;
				}
				"reviews_count" => {
					reviews_count_processing(pool.clone()).await.map_err(|e| {
						Box::new(std::io::Error::new(
							std::io::ErrorKind::Other,
							format!("{}", e),
						)) as Box<dyn std::error::Error + Send + Sync>
					})?;
				}
				"sitemap" => {
					sitemap_processing(pool.clone()).await.map_err(|e| {
						Box::new(std::io::Error::new(
							std::io::ErrorKind::Other,
							format!("{}", e),
						)) as Box<dyn std::error::Error + Send + Sync>
					})?;
				}
				"urls" => {
					urls_processing(pool.clone()).await.map_err(|e| {
						Box::new(std::io::Error::new(
							std::io::ErrorKind::Other,
							format!("{}", e),
						)) as Box<dyn std::error::Error + Send + Sync>
					})?;
				}
				"reviews" => {
					match oai_reviews_processing(pool.clone()).await {
						Ok(_) => {}
						Err(e) => {
							// Convert the Send + Sync error back to a standard error for the direct mode
							return Err(Box::new(std::io::Error::new(
								std::io::ErrorKind::Other,
								format!("{}", e),
							)) as Box<dyn std::error::Error + Send + Sync>);
						}
					}
				}
				"reviews_rewrite" => {
					match oai_reviews_rewrite_processing(pool.clone()).await {
						Ok(_) => {}
						Err(e) => {
							// Convert the Send + Sync error back to a standard error for the direct mode
							return Err(Box::new(std::io::Error::new(
								std::io::ErrorKind::Other,
								format!("{}", e),
							)) as Box<dyn std::error::Error + Send + Sync>);
						}
					}
				}
				"pages" => {
					match oai_pages_processing(pool.clone()).await {
						Ok(_) => {}
						Err(e) => {
							// Convert the Send + Sync error back to a standard error for the direct mode
							return Err(Box::new(std::io::Error::new(
								std::io::ErrorKind::Other,
								format!("{}", e),
							)) as Box<dyn std::error::Error + Send + Sync>);
						}
					}
				}
				"pages_sitemap" => {
					pages_sitemap_processing(pool.clone()).await.map_err(|e| {
						Box::new(std::io::Error::new(
							std::io::ErrorKind::Other,
							format!("{}", e),
						)) as Box<dyn std::error::Error + Send + Sync>
					})?;
				}

				_ => println!("error in env (no such handler)!"),
			}
		}
		"consumer" => {
			// New RabbitMQ consumer mode
			println!("Starting in RabbitMQ consumer mode...");
			start_rabbitmq_consumer().await.map_err(|e| {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("{}", e),
				)) as Box<dyn std::error::Error + Send + Sync>
			})?;
		}
		"result_consumer" => {
			// New RabbitMQ result consumer mode
			println!("Starting in RabbitMQ result consumer mode...");
			start_rabbitmq_result_consumer().await.map_err(|e| {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("{}", e),
				)) as Box<dyn std::error::Error + Send + Sync>
			})?;
		}
		"publisher" => {
			// New RabbitMQ publisher mode to send AI processing tasks
			println!("Starting in RabbitMQ publisher mode...");
			start_rabbitmq_publisher().await.map_err(|e| {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("{}", e),
				)) as Box<dyn std::error::Error + Send + Sync>
			})?;
		}
		_ => {
			eprintln!("Invalid RUN_MODE: {}. Use 'direct' or 'consumer'", run_mode);
			std::process::exit(1);
		}
	}

	Ok(())
}

async fn start_rabbitmq_result_consumer() -> Result<(), Box<dyn Error + Send + Sync>> {
	let rabbitmq_url = env::var("RABBITMQ_URL")
		.unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string());

	let queue_name =
		env::var("RABBITMQ_RESULT_QUEUE").unwrap_or_else(|_| "ai_processing_results".to_string());

	println!(
		"Starting RabbitMQ result consumer for queue: {}",
		queue_name
	);

	let consumer = RabbitMQConsumer::new(rabbitmq_url, queue_name);

	consumer
		.start_consuming_results(|result| Box::pin(handle_ai_processing_result(result)))
		.await
}

async fn handle_ai_processing_result(
	result: AIProcessingResult,
) -> Result<(), Box<dyn Error + Send + Sync>> {
	println!("🔍 Processing AI result for task: {}", result.task_id);
	println!("Status: {}", result.status);

	if let Some(ref result_data) = result.result_data {
		println!("Result data: {}", result_data);
	}

	if let Some(ref error_message) = result.error_message {
		eprintln!("Error in processing: {}", error_message);
	}

	// Here you can add custom logic to handle the result
	// For example, store in database, send to frontend, etc.
	println!("✅ AI result processed for task: {}", result.task_id);

	Ok(())
}

async fn start_rabbitmq_consumer() -> Result<(), Box<dyn Error + Send + Sync>> {
	let rabbitmq_url = env::var("RABBITMQ_URL")
		.unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string());

	let queue_name =
		env::var("RABBITMQ_QUEUE").unwrap_or_else(|_| "ai_processing_tasks".to_string());

	println!("Starting RabbitMQ consumer for queue: {}", queue_name);

	let consumer = RabbitMQConsumer::new(rabbitmq_url, queue_name);

	consumer
		.start_consuming(|task| Box::pin(handle_ai_processing_task(task)))
		.await
}

async fn handle_ai_processing_task(
	task: AIProcessingTask,
) -> Result<(), Box<dyn Error + Send + Sync>> {
	println!("🔍 Processing AI task: {}", task.task_id);
	println!("Processing type: {}", task.request_data.processing_type);

	// Create a RabbitMQ producer to send progress updates and final results
	let rabbitmq_url = env::var("RABBITMQ_URL")
		.unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string());
	let result_queue =
		env::var("RABBITMQ_RESULT_QUEUE").unwrap_or_else(|_| "ai_processing_results".to_string());

	let producer: Arc<Mutex<Option<RabbitMQProducer>>> = Arc::new(Mutex::new(
		match RabbitMQProducer::new(rabbitmq_url, result_queue).await {
			Ok(p) => Some(p),
			Err(e) => {
				eprintln!("⚠️ Failed to create RabbitMQ producer: {}", e);
				None
			}
		}
	));

	// Initialize database connection
	let config = Config::init();
	let pool = match sqlx::postgres::PgPoolOptions::new()
		.max_connections(10)
		.connect(&config.database_url)
		.await
	{
		Ok(pool) => {
			println!("✅ Connection to the database is successful!");
			pool
		}
		Err(err) => {
			eprintln!("🔥 Failed to connect to the database: {:?}", err);
			return Err(Box::new(std::io::Error::new(
				std::io::ErrorKind::Other,
				format!("{}", err),
			)) as Box<dyn std::error::Error + Send + Sync>);
		}
	};

	// Send initial progress update (skip for keyword_extraction)
	if task.request_data.processing_type != "keyword_extraction" {
		if let Some(ref prod) = *producer.lock().await {
			if let Err(e) = prod
				.send_progress_update(
					task.task_id,
					task.request_data.user_id,
					Some(task.request_data.request_id),
					0.0,
					"in_progress",
					"Task started",
				)
				.await
			{
				eprintln!("⚠️ Failed to send progress update: {}", e);
			}
		}
	}

	// Execute the appropriate processing based on task type using the original functions
	let processing_result: Result<String, Box<dyn std::error::Error + Send + Sync>> =
		match task.request_data.processing_type.as_str() {
			"description" => {
				match crate::oai_processing::oai_description_processing::process_description_with_qwen_cli(
					pool.clone(),
					&task,
				)
					.await
				{
					Ok(beautified_description) => Ok(beautified_description),
					Err(e) => Err(Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("{}", e),
					)) as Box<dyn std::error::Error + Send + Sync>),
				}
			}
			"title" => {
				match crate::oai_processing::oai_title_processing::process_title_with_qwen_cli(
					pool.clone(),
					&task,
				)
					.await
				{
					Ok(result) => Ok(result),
					Err(e) => Err(Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("{}", e),
					)) as Box<dyn std::error::Error + Send + Sync>),
				}
			}
			"keyword_extraction" => {
				match crate::oai_processing::keyword_extraction_processing::process_keyword_extraction_with_qwen_cli(
					pool.clone(),
					&task,
					producer.clone(),
				)
					.await
				{
					Ok(keywords) => Ok(keywords),
					Err(e) => Err(Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("{}", e),
					)) as Box<dyn std::error::Error + Send + Sync>),
				}
			}
			_ => {
				eprintln!(
					"❌ Unsupported processing type: {}",
					task.request_data.processing_type
				);
				Err(Box::new(std::io::Error::new(
					std::io::ErrorKind::InvalidInput,
					format!(
						"Unsupported processing type: {}",
						task.request_data.processing_type
					),
				)))
			}
		};

	// Send final result (except for keyword_extraction which sends its own results)
	// Skip sending result for keyword_extraction as it sends its own results
	if task.request_data.processing_type == "keyword_extraction" {
		println!("✅ Keyword extraction task completed, results already sent");
		return Ok(());
	}

	if let Some(ref prod) = *producer.lock().await {
		match processing_result {
			Ok(result_value) => {
				// Send the appropriate result data based on processing type
				let result_data = match task.request_data.processing_type.as_str() {
					"title" => json!({"beautified_title": result_value}),
					"description" => json!({"beautified_description": result_value}),
					_ => json!({"result": result_value}), // Default for other types
				};

				if let Err(e) = prod
					.send_result(
						task.task_id,
						task.request_data.user_id,
						Some(task.request_data.request_id),
						"completed",
						Some(result_data),
						None,
					)
					.await
				{
					eprintln!("⚠️ Failed to send completion result: {}", e);
				}
			}
			Err(e) => {
				if let Err(e) = prod
					.send_result(
						task.task_id,
						task.request_data.user_id,
						Some(task.request_data.request_id),
						"failed",
						None,
						Some(&e.to_string()),
					)
					.await
				{
					eprintln!("⚠️ Failed to send error result: {}", e);
				}
			}
		}
	}

	println!("✅ AI task completed: {}", task.task_id);
	Ok(())
}

async fn start_rabbitmq_publisher() -> Result<(), Box<dyn Error + Send + Sync>> {
	let rabbitmq_url = env::var("RABBITMQ_URL")
		.unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string());
	let publish_queue =
		env::var("RABBITMQ_PUBLISH_QUEUE").unwrap_or_else(|_| "ai_processing_tasks".to_string());

	println!("Starting RabbitMQ publisher...");

	let producer = match RabbitMQProducer::new(rabbitmq_url, publish_queue).await {
		Ok(p) => p,
		Err(e) => {
			eprintln!("Failed to create RabbitMQ producer: {}", e);
			return Err(e);
		}
	};

	// Example: Create and send an AI processing task
	let task = AIProcessingTask {
		task_id: Uuid::new_v4(),
		request_data: AIRequestData {
			request_id: Uuid::new_v4(),
			user_id: Uuid::new_v4(),
			processing_type: "description".to_string(), // Example processing type
			parameters: json!({"example_param": "example_value"}),
		},
		created_at: chrono::Utc::now().to_rfc3339(),
	};

	println!("Sending AI processing task: {}", task.task_id);
	match producer.send_ai_processing_task(&task).await {
		Ok(_) => {}
		Err(e) => {
			eprintln!("Failed to send AI processing task: {}", e);
			return Err(e);
		}
	};
	println!("✅ AI processing task sent successfully");

	// Keep the publisher alive for a short time to ensure message is sent
	sleep(Duration::from_secs(1)).await;

	Ok(())
}
