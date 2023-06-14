# cloudflare-dyndns-rs

Updates a Cloudflare DNS entry to the machine's external IP.

Use this on your home network to emulate "dyndns" features.

Written in Rust and distributed as a Docker image.

## Usage

```
Usage: cloudflare-dyndns-rs [OPTIONS] --token <TOKEN> <--zone-id <ID>|--zone-name <NAME>> <RECORD>

Arguments:
  <RECORD>
          The name of the DNS record to update ("example.com") [env:
          CLOUDFLARE_DNS_RECORD]

Options:
  -v, --verbose
          Increase log output to show what the application is doing
  -n, --dry-run
          Don't actually update the DNS record and instead only exit with the
          IP that would be written
  -h, --help
          Print help
  -V, --version
          Print version

Cloudflare:
  -t, --token <TOKEN>
          The Cloudflare API token [env: CLOUDFLARE_API_TOKEN]
      --zone-id <ID>
          The name of the zone to update ("6d3cf337c06d898fc4743293fda5ea3a")
          [env: CLOUDFLARE_ZONE_ID]
      --zone-name <NAME>
          The name of the zone to update ("example.com"). If no Zone ID is set,
          then this name is used to look up the Zone ID using the API [env:
          CLOUDFLARE_ZONE_NAME]
      --cloudflare-api-url <URL>
          Custom Cloudflare API base URL. Will use Cloudflare Production if not
          specified [env: CLOUDFLARE_API_URL]

IP:
      --ip-timeout <SECONDS>
          Request timeout for IP services [default: 5]
      --verify
          Talk to all available IP services and check that an absolute majority
          of them have the same answer before making any changes. Use this if
          you are extra paranoid and don't want a hacked or buggy service to be
          able to give you the wrong IP back
```

### Configuration

This utility supports configuration via command line argument, through ENV
variables, and through `.env` files. The `--help` output lists the named
environment variable to use for each option. CLI arguments override ENV
variables, when provided.

## License

Released under the MIT license. See `LICENSE` file.

Copyright (c) 2018-2023 Magnus Bergmark
