FROM docker.io/rust:latest as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo build --release

FROM docker.io/debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/target/release/muuf /usr/local/bin/muuf
CMD ["muuf","watch"]
