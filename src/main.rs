use cloudflare::{
    zones::dns::{DnsRecord, RecordType},
    Cloudflare,
};
use dotenv::dotenv;
use regex::Regex;
use std::collections::HashMap;
use structopt::StructOpt;

const DEFAULT_CLOUDFLARE_API_URL: &str = "https://api.cloudflare.com/client/v4/";
const IP_SERVICE_URLS: [&str; 8] = [
    // HTTPS sources
    "https://checkip.amazonaws.com/",
    "https://httpbin.org/ip",
    "https://icanhazip.com/",
    "https://ipecho.net/plain",
    "https://ipinfo.io/ip",
    // HTTP sources
    "http://checkip.dyndns.com/",
    "http://whatismyip.akamai.com/",
    "http://xur.io/ip",
];
const IPV4_MATCHER: &str = r#"\b\d{1,3}(\.\d{1,3}){3}\b"#;

#[derive(StructOpt)]
struct Options {
    /// Increase log output to show what the application is doing.
    #[structopt(long = "verbose", short = "v")]
    verbose: bool,

    /// Don't actually update the DNS record and instead only exit with the IP that would be
    /// written.
    #[structopt(long = "dry-run", short = "n")]
    dry_run: bool,

    /// Talk to all available IP services and check that an absolute majority of them have the same
    /// answer before making any changes. Use this if you are extra paranoid and don't want a
    /// hacked or buggy service to be able to give you the wrong IP back.
    #[structopt(long = "verify")]
    verify: bool,

    /// The Cloudflare account email.
    #[structopt(
        long = "email",
        short = "e",
        env = "CLOUDFLARE_API_EMAIL",
        value_name = "EMAIL"
    )]
    email: String,

    /// The Cloudflare API key.
    #[structopt(
        long = "key",
        short = "k",
        env = "CLOUDFLARE_API_KEY",
        value_name = "KEY"
    )]
    api_key: String,

    /// The name of the zone to update ("example.com")
    #[structopt(env = "CLOUDFLARE_ZONE_NAME", value_name = "NAME")]
    zone_name: String,

    /// The name of the DNS record to update ("example.com")
    #[structopt(env = "CLOUDFLARE_DNS_RECORD", value_name = "RECORD")]
    dns_record: String,

    /// Cloudflare API base URL. Default should work for all but the most specific cases. Note that
    /// this URL *must* end with a trailing slash.
    #[structopt(
        long = "cloudflare-api-url",
        env = "CLOUDFLARE_API_URL",
        value_name = "URL",
        raw(default_value = "DEFAULT_CLOUDFLARE_API_URL")
    )]
    base_url: String,
}

fn main() -> Result<(), String> {
    dotenv().ok();
    let options = Options::from_args();
    let cloudflare =
        Cloudflare::new(&options.api_key, &options.email, &options.base_url).map_err(|err| {
            format!(
                "Failed to initialize Cloudflare API client: {}",
                format_error(err)
            )
        })?;

    let zone_id = find_zone_id(&options, &cloudflare)?;
    let current_record = fetch_current_dns_record(&cloudflare, &zone_id, &options.dns_record)?;
    let ip = determine_external_ip(&options)?;

    if current_record.content == ip {
        eprintln!("Existing record is already correct. Exiting without changes.");
        Ok(())
    } else {
        if options.verbose {
            eprintln!(
                "IP difference: DNS is set to {dns}, while current IP is {current}",
                dns = current_record.content,
                current = ip
            );
        }

        if options.dry_run {
            eprintln!("Would update DNS record to point to {}", ip);
            Ok(())
        } else {
            update_dns_record(&cloudflare, &zone_id, current_record, ip)
        }
    }
}

fn find_zone_id(options: &Options, cloudflare: &Cloudflare) -> Result<String, String> {
    if options.verbose {
        eprint!("Resolving Zone ID… ");
    }

    let zone_id = cloudflare::zones::get_zoneid(cloudflare, &options.zone_name)
        .map_err(|err| format!("Failed to retreive zone ID: {}", format_error(err)))?;

    if options.verbose {
        eprintln!("OK. Found {}", zone_id);
    }

    Ok(zone_id)
}

fn fetch_current_dns_record(
    cloudflare: &Cloudflare,
    zone_id: &str,
    record_name: &str,
) -> Result<DnsRecord, String> {
    cloudflare::zones::dns::list_dns_of_type(cloudflare, zone_id, RecordType::A)
        .map_err(|err| format!("Failed to list DNS A records: {}", format_error(err)))
        .and_then(|list| {
            list.into_iter()
                .find(|record| record.name == record_name)
                .ok_or_else(|| format!("Could not find A record for {}", record_name))
        })
}

fn update_dns_record(
    cloudflare: &Cloudflare,
    zone_id: &str,
    current_record: DnsRecord,
    new_ip: String,
) -> Result<(), String> {
    use cloudflare::zones::dns::UpdateDnsRecord;

    cloudflare::zones::dns::update_dns_entry(
        cloudflare,
        zone_id,
        &current_record.id,
        &UpdateDnsRecord {
            record_type: current_record.record_type,
            name: current_record.name.clone(),
            content: new_ip,
            ttl: None,
            proxied: None,
        },
    )
    .map_err(|err| format!("Failed to update DNS record: {}", format_error(err)))
    .map(|_| ())
}

fn determine_external_ip(options: &Options) -> Result<String, String> {
    if options.verify {
        determine_external_ip_with_verification(options)
    } else {
        determine_external_ip_without_verification(options)
    }
}

fn determine_external_ip_without_verification(options: &Options) -> Result<String, String> {
    let matcher: Regex = IPV4_MATCHER
        .parse()
        .expect("Programmer error: Invalid regexp");

    if !options.verbose {
        eprint!("Retreiving external IP… ");
    }

    for url in IP_SERVICE_URLS.iter() {
        if options.verbose {
            eprint!("{} -> ", url);
        }

        let found_ip = reqwest::get(*url)
            .and_then(|mut result| result.text())
            .map(|body| extract_ip_from_body(&body, &matcher));

        match &found_ip {
            Ok(Some(ip)) => {
                eprintln!("{}", ip);
                return Ok(ip.clone());
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

fn determine_external_ip_with_verification(options: &Options) -> Result<String, String> {
    let matcher: Regex = IPV4_MATCHER
        .parse()
        .expect("Programmer error: Invalid regexp");

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

        let found_ip = reqwest::get(*url)
            .and_then(|mut result| result.text())
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
        0 => Err(format!("Error: All sources failed. Aborting")),
        1 => {
            let ip = votes.keys().next().unwrap();
            if options.verbose {
                eprintln!("All services agree on {}", ip);
            } else {
                eprintln!("Done");
            }
            Ok(ip.clone())
        }
        _ => {
            eprintln!("Warning: Some services disagree on IP!");
            let total_votes: u16 = votes.iter().map(|(_ip, tally)| *tally).sum();
            let top_vote = votes.iter().max_by_key(|(_ip, tally)| *tally).unwrap();
            // If the top vote got more than 2/3rds of the votes, it's in an absolute majority.
            if *top_vote.1 >= (total_votes * 2 / 3) {
                eprintln!(
                    "IP {ip} has absolute majority of the votes ({tally} of {total})",
                    ip = top_vote.0,
                    tally = top_vote.1,
                    total = votes.len()
                );
                Ok(top_vote.0.clone())
            } else {
                eprintln!("No IP has absolute majority:");
                for (ip, tally) in votes.iter() {
                    eprintln!("  {}: {}", ip, tally);
                }
                eprintln!("Aborting.");
                Err(format!("Could not determine IP"))
            }
        }
    }
}

fn extract_ip_from_body(body: &str, matcher: &Regex) -> Option<String> {
    matcher
        .captures(body)
        .map(|captures| captures[0].to_string())
}

fn format_error(error: cloudflare::Error) -> String {
    use cloudflare::Error;

    match error {
        Error::NoResultsReturned => "No results returned".into(),
        Error::InvalidOptions => "Invalid options".into(),
        Error::NotSuccess => "API request failed".into(),
        Error::Reqwest(cause) => format!("Network error: {}", cause),
        Error::Json(cause) => format!("JSON error: {}", cause),
        Error::Io(cause) => format!("IO error: {}", cause),
        Error::Url(cause) => format!("URL error: {}", cause),
    }
}
