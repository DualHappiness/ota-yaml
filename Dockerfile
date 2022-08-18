FROM rust as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
COPY data data
COPY --from=builder /usr/local/cargo/bin/ota-yaml /usr/local/bin/ota-yaml
RUN apt update && apt install -y vim && apt clean && rm -rf /var/lib/apt/lists/*
CMD ota-yaml
