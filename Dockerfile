FROM clux/muslrust:stable AS builder
MAINTAINER Magnus Bergmark "me@mange.dev"

COPY . .
RUN cargo test && cargo build --release

# Build app image
FROM alpine:latest
MAINTAINER Magnus Bergmark "me@mange.dev"

RUN apk --no-cache add ca-certificates

COPY --from=builder /volume/target/x86_64-unknown-linux-musl/release/cloudflare-dyndns-rs /usr/local/bin
CMD /usr/local/bin/cloudflare-dyndns-rs
