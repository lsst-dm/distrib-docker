FROM rust:1.87 AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
# Little trick I found to cache the dependencies. They take so long in gh linux/arm64
# If there are any issue remove the next 4 lines
RUN mkdir src/
RUN echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf target/ src/

COPY src/ ./src/

# Build the final release binary
RUN cargo build --release

# Stage 2: The Final Stage
FROM debian:latest

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 1001 eups \
    && useradd --uid 1001 --gid 1001 -m eups

WORKDIR /app

COPY --from=builder /app/target/release/gcs-indexer-rs /app/
RUN chown -R eups:eups /app


EXPOSE 8080

USER eups

# The command to run the application
CMD ["./gcs-indexer-rs","eups-prod"]

