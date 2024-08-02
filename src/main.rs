mod api;
mod config;
mod models;
mod oai_processing;
mod processing;
mod utils;

use config::Config;
use dotenv::dotenv;
use oai_processing::{oai_description_processing, oai_reviews_processing};
use processing::{
	images_processing, reviews_count_processing, sitemap_processing, urls_processing,
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
use tokio::time::{sleep, Duration};

#[feature(proc_macro_byte_character)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	dotenv().ok();

	if std::env::var_os("RUST_LOG").is_none() {
		std::env::set_var("RUST_LOG", "actix_web=info");
	}
	env_logger::init();

	let config = Config::init();
	println!("Starting command bot...");

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

	match processing_type.as_str() {
		"images" => images_processing().await?,
		"reviews_count" => reviews_count_processing(pool.clone()).await?,
		"sitemap" => sitemap_processing(pool.clone()).await?,
		"urls" => urls_processing(pool.clone()).await?,
		"description" => oai_description_processing(pool.clone()).await?,
		"reviews" => oai_reviews_processing(pool.clone()).await?,
		_ => println!("error in env (no such handler)!"),
	}

	Ok(())
}
