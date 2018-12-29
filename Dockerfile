FROM ekidd/rust-musl-builder AS builder
MAINTAINER Magnus Bergmark "magnus.bergmark@gmail.com"

# Create new blank project for our dependencies
RUN USER=root cargo init --bin --name cloudflare-dyndns-rs .

# Install dependencies and delete artifacts from the fake project.
COPY ./Cargo.* ./
RUN cargo build --tests && \
  cargo build --release && \
  rm -r ./src && \
  rm -f ./target/*/deps/cloudflare-dyndns-rs* ./target/*-musl/*/deps/cloudflare-dyndns-rs* && \
  rm -rf ./target/*/incremental ./target/*-musl/*/incremental

# Actually build this project, making sure tests pass first
COPY ./src ./src
# COPY ./tests ./tests
RUN cargo test && cargo build --release

# Build app image
FROM alpine:latest
MAINTAINER Magnus Bergmark "magnus.bergmark@gmail.com"

RUN apk --no-cache add ca-certificates

COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/cloudflare-dyndns-rs /usr/local/bin
CMD /usr/local/bin/cloudflare-dyndns-rs
