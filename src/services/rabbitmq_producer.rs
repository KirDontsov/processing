use crate::models::rabbitmq::{AIProcessingProgress, AIProcessingResult, AIProcessingTask};
use chrono::Utc;
use lapin::{options::*, types::FieldTable, Channel, Connection, ConnectionProperties};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct RabbitMQProducer {
	connection: Arc<Mutex<Option<Connection>>>,
	connection_string: String,
	publish_queue: String,
}

impl RabbitMQProducer {
	pub async fn new(
		connection_string: String,
		publish_queue: String,
	) -> Result<Self, Box<dyn Error + Send + Sync>> {
		let producer = Self {
			connection: Arc::new(Mutex::new(None)),
			connection_string,
			publish_queue,
		};

		// Initialize the connection
		producer.initialize_connection().await?;

		Ok(producer)
	}

	async fn initialize_connection(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
		let connection =
			Connection::connect(&self.connection_string, ConnectionProperties::default())
				.await
				.map_err(|e| -> Box<dyn Error + Send + Sync> {
					Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Connection error: {}", e),
					))
				})?;

		let mut conn_guard = self.connection.lock().await;
		*conn_guard = Some(connection);
		drop(conn_guard);

		Ok(())
	}

	async fn get_or_create_channel(&self) -> Result<Channel, Box<dyn Error + Send + Sync>> {
		// First, check if we have a valid connection without holding the lock during async operations
		{
			let conn_guard = self.connection.lock().await;
			if let Some(conn) = conn_guard.as_ref() {
				if conn.status().connected() {
					// Connection exists and is valid, we can create a channel
					let channel = conn.create_channel().await.map_err(
						|e| -> Box<dyn Error + Send + Sync> {
							Box::new(std::io::Error::new(
								std::io::ErrorKind::Other,
								format!("Channel error: {}", e),
							))
						},
					)?;

					// Declare queue to ensure it exists
					channel
						.queue_declare(
							&self.publish_queue,
							QueueDeclareOptions {
								durable: true,
								..Default::default()
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

					return Ok(channel);
				}
			}
		} // Release the lock before making the async connection call

		// If we get here, we need to establish a new connection
		let new_connection =
			Connection::connect(&self.connection_string, ConnectionProperties::default())
				.await
				.map_err(|e| -> Box<dyn Error + Send + Sync> {
					Box::new(std::io::Error::new(
						std::io::ErrorKind::Other,
						format!("Connection error: {}", e),
					))
				})?;

		// Now acquire the lock again to update the connection
		{
			let mut conn_guard = self.connection.lock().await;
			*conn_guard = Some(new_connection);
		} // Release the lock before creating the channel

		// Get the connection again to create the channel
		let conn_guard = self.connection.lock().await;
		let conn = conn_guard.as_ref().unwrap();
		let channel = conn
			.create_channel()
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Channel error: {}", e),
				))
			})?;

		// Declare queue to ensure it exists
		channel
			.queue_declare(
				&self.publish_queue,
				QueueDeclareOptions {
					durable: true,
					..Default::default()
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

		Ok(channel)
	}

	pub async fn send_message<T: Serialize + 'static>(
		&self,
		message: &T,
		routing_key: &str,
	) -> Result<(), Box<dyn Error + Send + Sync>> {
		let channel = self.get_or_create_channel().await?;

		let serialized_message =
			serde_json::to_string(message).map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Serialization error: {}", e),
				))
			})?;

		// Debug logging
		println!("Debug: Sending RabbitMQ message: {}", serialized_message);

		let payload = serialized_message.as_bytes();

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

		channel
			.basic_publish(
				"avito_exchange", // exchange name
				routing_key,      // routing key
				BasicPublishOptions::default(),
				payload,
				lapin::BasicProperties::default(),
			)
			.await
			.map_err(|e| -> Box<dyn Error + Send + Sync> {
				Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					format!("Publish error: {}", e),
				))
			})?;

		println!(
			"Debug: Successfully sent RabbitMQ message with routing key: {}",
			routing_key
		);

		Ok(())
	}

	pub async fn send_ai_processing_task(
		&self,
		task: &AIProcessingTask,
	) -> Result<(), Box<dyn Error + Send + Sync>> {
		self.send_message(task, &format!("task.{}", task.request_data.processing_type))
			.await
	}

	pub async fn send_result(
		&self,
		task_id: Uuid,
		user_id: Uuid,
		request_id: Option<Uuid>,
		status: &str,
		result_data: Option<serde_json::Value>,
		error_message: Option<&str>,
	) -> Result<(), Box<dyn Error + Send + Sync>> {
		let result_message = AIProcessingResult {
			task_id,
			user_id,
			request_id,
			status: status.to_string(),
			result_data,
			error_message: error_message.map(|s| s.to_string()),
			completed_at: Utc::now().to_rfc3339(),
		};

		// Use ai.result pattern to match what the main microservice is expecting
		let routing_key = format!("ai.result.{}", user_id);

		self.send_message(&result_message, &routing_key).await
	}

	pub async fn send_progress_update(
		&self,
		task_id: Uuid,
		user_id: Uuid,
		request_id: Option<Uuid>,
		progress: f64,
		status: &str,
		message: &str,
	) -> Result<(), Box<dyn Error + Send + Sync>> {
		let progress_message = AIProcessingProgress {
			task_id,
			user_id,
			request_id,
			progress,
			status: status.to_string(),
			message: message.to_string(),
			timestamp: Utc::now().to_rfc3339(),
		};

		// Use ai.progress pattern to match what the main microservice is expecting
		let routing_key = format!("ai.progress.{}", user_id);

		self.send_message(&progress_message, &routing_key).await
	}
}

impl Drop for RabbitMQProducer {
	fn drop(&mut self) {
		// We can't do async operations in Drop, so we won't close the connection here
		// The connection will be closed when the Arc goes out of scope
	}
}
