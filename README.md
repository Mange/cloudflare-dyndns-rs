# cloudflare-dyndns-rs

Updates a Cloudflare DNS entry to the machine's external IP. Run this on your
private network to have your home IP updating automatically, like with the
DynDNS functionality.

Written in Rust and distributed as a Docker image.

## Usage

```
cloudflare-dyndns-rs 0.1.0
Magnus Bergmark <magnus.bergmark@gmail.com>

USAGE:
    cloudflare-dyndns-rs [FLAGS] [OPTIONS] <NAME> <RECORD> --key <KEY> --email <EMAIL>

FLAGS:
    -n, --dry-run    Don't actually update the DNS record and instead only exit with the IP that would be written.
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Increase log output to show what the application is doing.
        --verify     Talk to all available IP services and check that an absolute majority of them have the same answer
                     before making any changes. Use this if you are extra paranoid and don't want a hacked or buggy
                     service to be able to give you the wrong IP back.

OPTIONS:
    -k, --key <KEY>                   The Cloudflare API key. [env: CLOUDFLARE_API_KEY=]
        --cloudflare-api-url <URL>    Cloudflare API base URL. Default should work for all but the most specific cases.
                                      Note that this URL *must* end with a trailing slash. [env: CLOUDFLARE_API_URL=]
                                      [default: https://api.cloudflare.com/client/v4/]
    -e, --email <EMAIL>               The Cloudflare account email. [env: CLOUDFLARE_API_EMAIL=]
        --ip-timeout <SECONDS>        Request timeout for IP services. [default: 5]

ARGS:
    <NAME>      The name of the zone to update ("example.com") [env: CLOUDFLARE_ZONE_NAME=]
    <RECORD>    The name of the DNS record to update ("example.com") [env: CLOUDFLARE_DNS_RECORD=]
```

### Configuration

This utility supports configuration via command line argument, through ENV
variables, and through `.env` files. The `--help` output lists the named
environment variable to use for each option. CLI arguments override ENV
variables, when provided.

## License

Released under the MIT license. See `LICENSE` file.

Copyright (c) 2018 Magnus Bergmark
