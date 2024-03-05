use clap::{Args, Parser};
use cloudflare::endpoints::dns::{self, DnsContent};
use cloudflare::endpoints::zone;
use cloudflare::framework::auth::Credentials;
use cloudflare::framework::response::{ApiErrors, ApiFailure};
use cloudflare::framework::{Environment, HttpApiClientConfig};
use cloudflare::{endpoints::dns::DnsRecord, framework::HttpApiClient as CloudflareClient};
use dotenv::dotenv;
use regex::Regex;
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::Url;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::Duration;

const IP_SERVICE_URLS: [&str; 7] = [
    // HTTPS sources
    "https://checkip.amazonaws.com/",
    "https://httpbin.org/ip",
    "https://icanhazip.com/",
    "https://ipecho.net/plain",
    "https://ipinfo.io/ip",
    // HTTP sources
    "http://checkip.dyndns.com/",
    "http://whatismyip.akamai.com/",
];
const IPV4_MATCHER: &str = r"\b\d{1,3}(\.\d{1,3}){3}\b";

#[derive(Parser, Debug)]
#[command(
    author,
    about,
    version,
    next_line_help = true,
    args_override_self = true
)]
struct Options {
    /// Increase log output to show what the application is doing.
    #[arg(long = "verbose", short = 'v')]
    verbose: bool,

    /// Don't actually update the DNS record and instead only exit with the IP that would be
    /// written.
    #[arg(long = "dry-run", short = 'n')]
    dry_run: bool,

    /// The Cloudflare API token.
    #[arg(
        long = "token",
        short = 't',
        env = "CLOUDFLARE_API_TOKEN",
        value_name = "TOKEN",
        help_heading = "Cloudflare"
    )]
    api_token: String,

    #[command(flatten)]
    zone_options: ZoneOptions,

    /// The name of the DNS record to update ("example.com")
    #[arg(env = "CLOUDFLARE_DNS_RECORD", value_name = "RECORD")]
    dns_record: String,

    /// Custom Cloudflare API base URL. Will use Cloudflare Production if not specified.
    #[arg(
        long = "cloudflare-api-url",
        env = "CLOUDFLARE_API_URL",
        value_name = "URL",
        help_heading = "Cloudflare"
    )]
    base_url: Option<Url>,

    /// Request timeout for IP services.
    #[arg(
        long = "ip-timeout",
        value_name = "SECONDS",
        default_value = "5",
        help_heading = "IP"
    )]
    ip_timeout: u16,

    /// Talk to all available IP services and check that an absolute majority of them have the same
    /// answer before making any changes. Use this if you are extra paranoid and don't want a
    /// hacked or buggy service to be able to give you the wrong IP back.
    #[arg(long = "verify", help_heading = "IP")]
    verify: bool,
}

#[derive(Args, Debug)]
#[group(required = true, multiple = true)]
struct ZoneOptions {
    /// The name of the zone to update ("6d3cf337c06d898fc4743293fda5ea3a").
    #[arg(
        long = "zone-id",
        env = "CLOUDFLARE_ZONE_ID",
        value_name = "ID",
        help_heading = "Cloudflare"
    )]
    id: Option<String>,

    /// The name of the zone to update ("example.com"). If no Zone ID is set, then this name is
    /// used to look up the Zone ID using the API.
    #[arg(
        long = "zone-name",
        env = "CLOUDFLARE_ZONE_NAME",
        value_name = "NAME",
        help_heading = "Cloudflare"
    )]
    name: Option<String>,
}

impl Options {
    fn cloudflare_credentials(&self) -> Credentials {
        Credentials::UserAuthToken {
            token: self.api_token.clone(),
        }
    }

    fn cloudflare_environment(&self) -> Environment {
        match &self.base_url {
            Some(url) => Environment::Custom(url.to_owned()),
            None => Environment::Production,
        }
    }
}

fn main() -> Result<(), String> {
    dotenv().ok();
    let options = Options::parse();

    if options.ip_timeout == 0 {
        return Err(String::from(
            "A timeout of 0 seconds would mean no request could ever work.",
        ));
    }

    let cloudflare = CloudflareClient::new(
        options.cloudflare_credentials(),
        HttpApiClientConfig::default(),
        options.cloudflare_environment(),
    )
    .map_err(|err| format!("Failed to initialize Cloudflare API client: {}", err))?;

    let zone_id = find_zone_id(&options, &cloudflare)?;

    let current_record = fetch_current_dns_record(&cloudflare, &zone_id, &options.dns_record)?;
    let external_ip = determine_external_ip(&options)?;

    match current_record.content {
        DnsContent::A { content: ip } if ip == external_ip => {
            eprintln!("Existing record is already correct. Exiting without changes.");
            Ok(())
        }
        _ => {
            if options.verbose {
                eprintln!(
                    "IP difference: DNS is set to {dns:?}, while current IP is {current}",
                    dns = current_record.content,
                    current = external_ip
                );
            }

            if options.dry_run {
                eprintln!("Would update DNS record to point to {}", external_ip);
                Ok(())
            } else {
                update_dns_record(&cloudflare, &zone_id, current_record, external_ip)
            }
        }
    }
}

fn find_zone_id(options: &Options, cloudflare: &CloudflareClient) -> Result<String, String> {
    if let Some(id) = &options.zone_options.id {
        return Ok(id.to_owned());
    }

    let name = options
        .zone_options
        .name
        .as_ref()
        .ok_or_else(|| "Neither Zone ID or Zone Name was specified".to_string())?;

    if options.verbose {
        eprint!("Resolving Zone ID… ");
    }

    let zones = cloudflare
        .request(&zone::ListZones {
            params: zone::ListZonesParams {
                name: Some(name.to_owned()),
                ..Default::default()
            },
        })
        .map_err(|err| {
            dbg!(&err);
            format!(
                "Failed to retreive zone ID: {}",
                format_cloudflare_api_failure(err)
            )
        })?
        .result;

    let zone = zones
        .into_iter()
        .find(|zone| &zone.name == name)
        .ok_or_else(|| {
            format!(
                "Failed to retrieve zone ID: No ones with name {} found",
                name
            )
        })?;

    if options.verbose {
        eprintln!("OK. Found {}", zone.id);
    }

    Ok(zone.id)
}

fn fetch_current_dns_record(
    cloudflare: &CloudflareClient,
    zone_id: &str,
    record_name: &str,
) -> Result<DnsRecord, String> {
    let request = dns::ListDnsRecords {
        zone_identifier: zone_id,
        params: dns::ListDnsRecordsParams {
            name: Some(record_name.to_owned()),
            ..Default::default()
        },
    };

    let records = cloudflare
        .request(&request)
        .map_err(|err| {
            format!(
                "Failed to list DNS records for zone {}: {}",
                zone_id,
                format_cloudflare_api_failure(err)
            )
        })?
        .result;

    records
        .into_iter()
        .find(|record| record.name == record_name)
        .ok_or_else(|| format!("Could not find A record for {}", record_name))
}

fn update_dns_record(
    cloudflare: &CloudflareClient,
    zone_id: &str,
    current_record: DnsRecord,
    new_ip: Ipv4Addr,
) -> Result<(), String> {
    let request = dns::UpdateDnsRecord {
        zone_identifier: zone_id,
        identifier: &current_record.id,
        params: dns::UpdateDnsRecordParams {
            name: &current_record.name,
            content: DnsContent::A { content: new_ip },
            ttl: None,
            proxied: None,
        },
    };

    cloudflare
        .request(&request)
        .map_err(|err| {
            format!(
                "Failed to update DNS record: {}",
                format_cloudflare_api_failure(err)
            )
        })
        .map(|_| ())
}

fn http_client(options: &Options) -> Result<Client, String> {
    ClientBuilder::new()
        .timeout(Duration::from_secs(options.ip_timeout.into()))
        .build()
        .map_err(|error| format!("Failed to construct HTTP client: {}", error))
}

fn determine_external_ip(options: &Options) -> Result<Ipv4Addr, String> {
    if options.verify {
        determine_external_ip_with_verification(options)
    } else {
        determine_external_ip_without_verification(options)
    }
}

fn parse_ip(string: &str) -> Result<Ipv4Addr, String> {
    string
        .parse()
        .map_err(|err| format!("Failed to parse IP address {}: {}", string, err))
}

fn determine_external_ip_without_verification(options: &Options) -> Result<Ipv4Addr, String> {
    let matcher: Regex = IPV4_MATCHER
        .parse()
        .expect("Programmer error: Invalid regexp");
    let client = http_client(options)?;

    if !options.verbose {
        eprint!("Retreiving external IP… ");
    }

    for url in IP_SERVICE_URLS.iter() {
        if options.verbose {
            eprint!("{} -> ", url);
        }

        let found_ip = client
            .get(*url)
            .send()
            .and_then(|result| result.text())
            .map(|body| extract_ip_from_body(&body, &matcher));

        match &found_ip {
            Ok(Some(ip)) => {
                eprintln!("{}", ip);
                return parse_ip(ip);
            }
            Ok(None) => {
                if options.verbose {
                    eprintln!("Failed. No IP found in response.")
                }
            }
            Err(err) => {
                if options.verbose {
                    eprintln!("Failed. {}", err)
                }
            }
        }
    }

    Err(format!(
        "None of the {} service(s) replied successfully.",
        IP_SERVICE_URLS.len()
    ))
}

fn determine_external_ip_with_verification(options: &Options) -> Result<Ipv4Addr, String> {
    let matcher: Regex = IPV4_MATCHER
        .parse()
        .expect("Programmer error: Invalid regexp");
    let client = http_client(options)?;

    let mut votes: HashMap<String, u16> = HashMap::new();

    let longest_url_length = IP_SERVICE_URLS
        .iter()
        .map(|url| url.len())
        .max()
        .unwrap_or(10);

    if !options.verbose {
        eprint!("Retreiving and validating external IP… ");
    }

    for url in IP_SERVICE_URLS.iter() {
        if options.verbose {
            eprint!("{0:>1$} -> ", url, longest_url_length);
        }

        let found_ip = client
            .get(*url)
            .send()
            .and_then(|result| result.text())
            .map(|body| extract_ip_from_body(&body, &matcher));

        if options.verbose {
            match &found_ip {
                Ok(Some(ip)) => eprintln!("{}", ip),
                Ok(None) => eprintln!("Failed. No IP found in response."),
                Err(err) => eprintln!("Failed. {}", err),
            }
        }

        if let Ok(Some(ip)) = found_ip {
            *votes.entry(ip).or_insert(0) += 1;
        }
    }

    match votes.len() {
        0 => Err("Error: All sources failed. Aborting".to_string()),
        1 => {
            let ip = votes.keys().next().unwrap();
            if options.verbose {
                eprintln!("All services agree on {}", ip);
            } else {
                eprintln!("Done");
            }
            parse_ip(ip)
        }
        _ => {
            eprintln!("Warning: Some services disagree on IP!");
            let total_votes: u16 = votes.values().copied().sum();
            let top_vote = votes.iter().max_by_key(|(_ip, tally)| *tally).unwrap();
            // If the top vote got more than 2/3rds of the votes, it's in an absolute majority.
            if *top_vote.1 >= (total_votes * 2 / 3) {
                eprintln!(
                    "IP {ip} has absolute majority of the votes ({tally} of {total})",
                    ip = top_vote.0,
                    tally = top_vote.1,
                    total = votes.len()
                );
                parse_ip(top_vote.0)
            } else {
                eprintln!("No IP has absolute majority:");
                for (ip, tally) in votes.iter() {
                    eprintln!("  {}: {}", ip, tally);
                }
                eprintln!("Aborting.");
                Err("Could not determine IP".to_string())
            }
        }
    }
}

fn extract_ip_from_body(body: &str, matcher: &Regex) -> Option<String> {
    matcher
        .captures(body)
        .map(|captures| captures[0].to_string())
}

fn format_cloudflare_api_failure(failure: ApiFailure) -> String {
    match failure {
        ApiFailure::Error(status, errors) => format!(
            "Status code {status}:\n  {errors}",
            status = status,
            errors = format_cloudflare_errors(errors),
        ),
        ApiFailure::Invalid(err) => err.to_string(),
    }
}

fn format_cloudflare_errors(errors: ApiErrors) -> String {
    errors
        .errors
        .iter()
        .map(|error| format!("{}: {}", error.code, error.message))
        .collect::<Vec<String>>()
        .join("\n  ")
}
