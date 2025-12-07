use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AIRequestData {
	pub request_id: Uuid,
	pub user_id: Uuid,
	pub processing_type: String, // "description", "reviews", "pages", etc.
	pub parameters: serde_json::Value, // Additional parameters for the specific processing
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AIProcessingTask {
	pub task_id: Uuid,
	pub request_data: AIRequestData,
	pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AIProcessingResult {
	pub task_id: Uuid,
	pub user_id: Uuid,
	pub request_id: Option<Uuid>,
	pub status: String, // "completed", "failed", "cancelled"
	pub result_data: Option<serde_json::Value>,
	pub error_message: Option<String>,
	pub completed_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AIProcessingProgress {
	pub task_id: Uuid,
	pub user_id: Uuid,
	pub request_id: Option<Uuid>,
	pub progress: f64,
	pub status: String, // "in_progress", "completed", "error"
	pub message: String,
	pub timestamp: String,
}
