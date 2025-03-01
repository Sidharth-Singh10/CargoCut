FROM rust:1.82.0-slim as builder
WORKDIR /usr/src/app

# Copy entire project
COPY . .

# Build the application
RUN cargo build --release

FROM debian:bookworm-slim
# Install runtime dependencies in one layer
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl3 \
    libpq5 \
    ca-certificates && \
    rm -rf /var/lib/apt/lists/*
    
WORKDIR /app

# Copy the correct binary name from the builder stage
COPY --from=builder /usr/src/app/target/release/cargocut /app/cargocut
# Copy migrations folder for database setup
COPY --from=builder /usr/src/app/migrations /app/migrations/

# Use non-root user for security
RUN useradd -m appuser && chown -R appuser:appuser /app
USER appuser

EXPOSE 3001
ENV DATABASE_URL=example
ENV REDIS_URL=redis://localhost:6379


# Run migrations and start the application
CMD ["./cargocut"]