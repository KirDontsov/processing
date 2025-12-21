# Topvybor processing

## Running with Docker

The application can be run using Docker and Docker Compose. This approach provides an isolated environment with all dependencies.

### Prerequisites

- Docker
- Docker Compose

### Running the Application

```bash
# Build and start all services (application, PostgreSQL, RabbitMQ)
docker-compose up -d

# View logs
docker-compose logs -f

# Stop services
docker-compose down
```

### Docker Compose Services

The `docker-compose.yml` defines the following services:

- `postgres`: PostgreSQL database server
- `rabbitmq`: RabbitMQ message broker
- `processing-consumer`: Main application in consumer mode (processes AI tasks from queue)
- `processing-result-consumer`: Result consumer service (handles processing results)
- `processing-publisher`: Publisher service (optional, for testing task creation)

### Running in Different Modes

The application supports different run modes via the `RUN_MODE` environment variable:

- `consumer`: Process tasks from RabbitMQ queue
- `result_consumer`: Handle results from RabbitMQ queue
- `publisher`: Publish tasks to RabbitMQ queue
- `direct`: Direct execution mode (requires additional configuration)

### Build Troubleshooting

The Dockerfile now uses Rust nightly to support the `edition2024` feature required by one of the dependencies. If you encounter any issues with the nightly build, you can:

1. Build and run the application locally instead of using Docker

To build locally:
```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install sqlx-cli if not already installed
cargo install sqlx-cli

# Build the application
cargo build --release

# Run with different modes
RUN_MODE=consumer cargo run --release
```

## Database Migrations

Before running the application, ensure database migrations are applied:

```bash
# Run database migrations
sqlx migrate run
```

Note: The application uses sqlx for compile-time verification of SQL queries, which requires the database schema to be present during compilation. If you encounter build errors related to SQL queries, make sure your database is running and migrations have been applied.

## Running Locally (Traditional Method)

```docker-compose up -d``` - start PostgreSQL server

```cargo install sqlx-cli``` - if not installed

```sqlx migrate run``` - migration script

```cargo watch -q -c -w src/ -x run``` - run for dev

```cargo r -r``` - run for prod

```./chromedriver --port=9515 --disable-gpu --dns-prefetch-disable --disable-extensions --no-sandbox enable-automation``` - run chrome driver, если вылетает, нужно обновить на более новую версию chromedriver-mac-x64
