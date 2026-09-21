FROM ghcr.io/blackdex/rust-musl:x86_64-musl-stable AS builder

COPY . .
RUN cargo test && cargo build --release

# Build app image
FROM alpine:latest

RUN apk --no-cache add ca-certificates

COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/cloudflare-dyndns-rs /usr/local/bin
CMD ["/usr/local/bin/cloudflare-dyndns-rs"]
