use crate::models::rabbitmq::{
	AIProcessingProgress, AIProcessingResult, AIProcessingTask, AIRequestData,
};
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;
use lapin::{message::Delivery, options::*, types::FieldTable, Connection, ConnectionProperties};
use serde::{Deserialize, Serialize};
use std::error::Error;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct LegacyTitleTaskFormat {
	task_id: Uuid,
	user_id: Uuid,
	title: String,
	category: String,
	created_ts: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct LegacyDescriptionTaskFormat {
	task_id: Uuid,
	user_id: Uuid,
	description: String,
	category: String,
	created_ts: DateTime<Utc>,
}

pub struct RabbitMQConsumer {
	connection_string: String,
	queue_name: String,
}

impl RabbitMQConsumer {
	pub fn new(connection_string: String, queue_name: String) -> Self {
		Self {
			connection_string,
			queue_name,
		}
	}

	pub async fn start_consuming<F>(
		&self,
		message_handler: F,
	) -> Result<(), Box<dyn Error + Send + Sync>>
	where
		F: Fn(
				AIProcessingTask,
			) -> futures::future::BoxFuture<'static, Result<(), Box<dyn Error + Send + Sync>>>
			+ Send
			+ Sync
			+ 'static,
	{
		println!("Connecting to RabbitMQ at: {}", self.connection_string);

		let connection =
			Connection::connect(&self.connection_string, ConnectionProperties::default())
				.await
				.map_err(|e| -> Box<dyn Error + Send + Sync> {
					Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Connection error: {}", e),
					))
				})?;

		println!("✅ Connected to RabbitMQ");

		let channel =
			connection
				.create_channel()
				.await
				.map_err(|e| -> Box<dyn Error + Send + Sync> {
					Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Channel error: {}", e),
					))
				})?;

		// Declare exchange
		channel
			.exchange_declare(
				"avito_exchange",
				lapin::ExchangeKind::Topic,
				ExchangeDeclareOptions {
					durable: true,
					..ExchangeDeclareOptions::default()
				},
				FieldTable::default(),
			)
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Exchange declaration error: {}", e),
				))
			})?;

		// Create the processing tasks queue
		let queue = channel
			.queue_declare(
				&self.queue_name,
				QueueDeclareOptions {
					durable: true,
					exclusive: false,
					auto_delete: false,
					..QueueDeclareOptions::default()
				},
				FieldTable::default(),
			)
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Queue error: {}", e),
				))
			})?;

		// Bind the queue to the exchange with a pattern to receive AI processing tasks
		channel
			.queue_bind(
				queue.name().as_str(),
				"avito_exchange",
				"task.*", // Binding pattern to receive processing tasks
				QueueBindOptions::default(),
				FieldTable::default(),
			)
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Queue binding error: {}", e),
				))
			})?;

		println!(
			"📋 Queue '{}' declared with {} messages waiting",
			queue.name(),
			queue.message_count()
		);

		// Set up fair dispatch - each consumer processes one message at a time
		channel
			.basic_qos(1, lapin::options::BasicQosOptions::default())
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("QoS error: {}", e),
				))
			})?;

		let mut consumer = channel
			.basic_consume(
				queue.name().as_str(),
				"ai_processing_consumer", // Consumer tag
				BasicConsumeOptions::default(),
				FieldTable::default(),
			)
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Consumer error: {}", e),
				))
			})?;

		println!("🚀 Consumer started. Waiting for messages...");

		while let Some(delivery) = consumer.next().await {
			match delivery {
				Ok(delivery) => {
					println!("📨 Received message");
					self.handle_delivery(delivery, &message_handler).await?;
				}
				Err(e) => {
					eprintln!("❌ Error receiving delivery: {}", e);
					return Err(Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Delivery error: {}", e),
					)));
				}
			}
		}

		Ok(())
	}

	pub async fn start_consuming_results<F>(
		&self,
		result_handler: F,
	) -> Result<(), Box<dyn Error + Send + Sync>>
	where
		F: Fn(
				AIProcessingResult,
			) -> futures::future::BoxFuture<'static, Result<(), Box<dyn Error + Send + Sync>>>
			+ Send
			+ Sync
			+ 'static,
	{
		println!("Connecting to RabbitMQ at: {}", self.connection_string);

		let connection =
			Connection::connect(&self.connection_string, ConnectionProperties::default())
				.await
				.map_err(|e| -> Box<dyn Error + Send + Sync> {
					Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Connection error: {}", e),
					))
				})?;

		println!("✅ Connected to RabbitMQ for results");

		let channel =
			connection
				.create_channel()
				.await
				.map_err(|e| -> Box<dyn Error + Send + Sync> {
					Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Channel error: {}", e),
					))
				})?;

		// Declare exchange
		channel
			.exchange_declare(
				"avito_exchange",
				lapin::ExchangeKind::Topic,
				ExchangeDeclareOptions {
					durable: true,
					..ExchangeDeclareOptions::default()
				},
				FieldTable::default(),
			)
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Exchange declaration error: {}", e),
				))
			})?;

		// Create the results queue
		let queue = channel
			.queue_declare(
				&self.queue_name,
				QueueDeclareOptions {
					durable: true,
					exclusive: false,
					auto_delete: false,
					..QueueDeclareOptions::default()
				},
				FieldTable::default(),
			)
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Queue error: {}", e),
				))
			})?;

		// Bind the queue to the exchange with a pattern to receive AI processing results
		channel
			.queue_bind(
				queue.name().as_str(),
				"avito_exchange",
				"result.*", // Binding pattern to receive processing results
				QueueBindOptions::default(),
				FieldTable::default(),
			)
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Queue binding error: {}", e),
				))
			})?;

		println!(
			"📋 Results queue '{}' declared with {} messages waiting",
			queue.name(),
			queue.message_count()
		);

		// Set up fair dispatch - each consumer processes one message at a time
		channel
			.basic_qos(1, lapin::options::BasicQosOptions::default())
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("QoS error: {}", e),
				))
			})?;

		let mut consumer = channel
			.basic_consume(
				queue.name().as_str(),
				"ai_processing_result_consumer", // Consumer tag
				BasicConsumeOptions::default(),
				FieldTable::default(),
			)
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Consumer error: {}", e),
				))
			})?;

		println!("🚀 Result consumer started. Waiting for result messages...");

		while let Some(delivery) = consumer.next().await {
			match delivery {
				Ok(delivery) => {
					println!("📨 Received result message");
					self.handle_result_delivery(delivery, &result_handler)
						.await?;
				}
				Err(e) => {
					eprintln!("❌ Error receiving result delivery: {}", e);
					return Err(Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Delivery error: {}", e),
					)));
				}
			}
		}

		Ok(())
	}

	async fn handle_delivery<F>(
		&self,
		delivery: Delivery,
		message_handler: &F,
	) -> Result<(), Box<dyn Error + Send + Sync>>
	where
		F: Fn(
				AIProcessingTask,
			) -> futures::future::BoxFuture<'static, Result<(), Box<dyn Error + Send + Sync>>>
			+ Send
			+ Sync
			+ 'static,
	{
		let message_str =
			std::str::from_utf8(&delivery.data).map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("UTF8 error: {}", e),
				))
			})?;

		println!("Raw message: {}", message_str);

		// Try to parse as AIProcessingTask first
		let task: AIProcessingTask = match serde_json::from_str::<AIProcessingTask>(message_str) {
			Ok(task) => task,
			Err(_) => {
				// If that fails, try to parse as the legacy title format and convert it
				match serde_json::from_str::<LegacyTitleTaskFormat>(message_str) {
					Ok(legacy_task) => {
						// Convert the legacy format to the new AIProcessingTask format
						AIProcessingTask {
							task_id: legacy_task.task_id,
							request_data: AIRequestData {
								request_id: legacy_task.task_id, // Use task_id as request_id for compatibility
								user_id: legacy_task.user_id,
								processing_type: "title".to_string(), // Default to title processing
								parameters: serde_json::json!({
									"title": legacy_task.title,
									"category": legacy_task.category,
									"created_ts": legacy_task.created_ts
								}),
							},
							created_at: legacy_task.created_ts.to_rfc3339(),
						}
					}
					Err(_) => {
						// If that fails, try to parse as the legacy description format and convert it
						match serde_json::from_str::<LegacyDescriptionTaskFormat>(message_str) {
							Ok(legacy_desc_task) => {
								// Convert the legacy format to the new AIProcessingTask format
								AIProcessingTask {
									task_id: legacy_desc_task.task_id,
									request_data: AIRequestData {
										request_id: legacy_desc_task.task_id, // Use task_id as request_id for compatibility
										user_id: legacy_desc_task.user_id,
										processing_type: "description".to_string(), // Set to description processing
										parameters: serde_json::json!({
											"description": legacy_desc_task.description,
											"category": legacy_desc_task.category,
											"created_ts": legacy_desc_task.created_ts
										}),
									},
									created_at: legacy_desc_task.created_ts.to_rfc3339(),
								}
							}
							Err(e) => {
								eprintln!("Failed to parse message as AIProcessingTask, legacy title format, or legacy description format: {}", e);
								eprintln!("Message content: {}", message_str);
								return Err(Box::new(std::io::Error::new(
									std::io::ErrorKind::Other,
									format!("Parse error: {}", e),
								)));
							}
						}
					}
				}
			}
		};

		println!("🎯 Parsed AI processing task: {}", task.task_id);

		// Process the message
		message_handler(task).await?;

		// Acknowledge the message
		delivery.ack(BasicAckOptions::default()).await.map_err(
			|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Ack error: {}", e),
				))
			},
		)?;

		println!("✅ Message acknowledged");
		Ok(())
	}

	async fn handle_result_delivery<F>(
		&self,
		delivery: Delivery,
		result_handler: &F,
	) -> Result<(), Box<dyn Error + Send + Sync>>
	where
		F: Fn(
				AIProcessingResult,
			) -> futures::future::BoxFuture<'static, Result<(), Box<dyn Error + Send + Sync>>>
			+ Send
			+ Sync
			+ 'static,
	{
		let message_str =
			std::str::from_utf8(&delivery.data).map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("UTF8 error: {}", e),
				))
			})?;

		println!("Raw result message: {}", message_str);

		// Parse as AIProcessingResult
		let result: AIProcessingResult =
			match serde_json::from_str::<AIProcessingResult>(message_str) {
				Ok(result) => result,
				Err(e) => {
					eprintln!(
						"Failed to parse result message as AIProcessingResult: {}",
						e
					);
					eprintln!("Message content: {}", message_str);
					return Err(Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Parse error: {}", e),
					)));
				}
			};

		println!("🎯 Parsed AI processing result: {}", result.task_id);

		// Process the result
		result_handler(result).await?;

		// Acknowledge the message
		delivery.ack(BasicAckOptions::default()).await.map_err(
			|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Ack error: {}", e),
				))
			},
		)?;

		println!("✅ Result message acknowledged");
		Ok(())
	}
}
