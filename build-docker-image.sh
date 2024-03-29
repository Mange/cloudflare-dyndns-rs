#!/usr/bin/env bash
# Build the latest version of the Docker image

set -e

version=$(
  grep 'version =' Cargo.toml | head -n1 | awk '{ print $3 }' | tr -d '"'
)
image_name=mange/cloudflare-dyndns-rs:${version}
latest_name=mange/cloudflare-dyndns-rs:latest

echo -n "Will build and push ${image_name} and ${latest_name}. Press enter to continue."
read -r

docker build -t "$image_name" .
docker tag "$image_name" "$latest_name"
docker push "$image_name"
docker push "$latest_name"
