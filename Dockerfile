FROM rust:1.75 as builder

WORKDIR /usr/src/app
COPY . .

# Install sqlx-cli for migrations
RUN cargo install sqlx-cli --no-default-features --features postgres

# Build the application with release profile
RUN cargo build --release

# Create a smaller image for the runtime
FROM debian:bookworm-slim

# Install required dependencies for SSL and PostgreSQL
RUN apt-get update && apt-get install -y \
    libssl-dev \
    libpq-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from the builder stage
COPY --from=builder /usr/src/app/target/release/url-shortener /app/url-shortener
# Copy migrations folder for database setup
COPY --from=builder /usr/src/app/migrations /app/migrations

# Expose the port your application runs on
EXPOSE 8080

# Set environment variables (these will be overridden in k8s deployment)
ENV DATABASE_URL=postgres://postgres:postgres@localhost:5432/urlshortener
ENV REDIS_URL=redis://localhost:6379

# Run migrations and start the application
CMD sqlx migrate run && ./url-shortener