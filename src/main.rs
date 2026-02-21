use clap::Parser;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

#[derive(Parser, Debug)]
#[command(name = "cidrfest")]
#[command(
    about = "Filter IP geolocation data by country codes",
    long_about = "Reads optional config.toml from the current working directory. CLI arguments override config.toml values."
)]
struct Args {
    /// Country codes to filter (can be specified multiple times)
    #[arg(short = 'c', long = "country", help = "Country code to filter (required unless provided via config.toml; can be specified multiple times)")]
    country_codes: Vec<String>,

    /// ASN numbers to include (can be specified multiple times)
    #[arg(short = 'a', long = "asn", help = "ASN number to include (optional; can be specified multiple times; overrides config.toml if provided)")]
    asn_numbers: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct Config {
    country_codes: Option<Vec<String>>,
    asn_numbers: Option<Vec<String>>,
    file_url: Option<String>,
    sha256_url: Option<String>,
    local_file_path: Option<String>,
    local_file_cidr: Option<String>,
    asn_base_url: Option<String>,
}

struct AppConfig {
    country_codes: Vec<String>,
    asn_numbers: Vec<String>,
    file_url: String,
    sha256_url: String,
    local_file_path: String,
    local_file_cidr: String,
    asn_base_url: String,
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let path = Path::new("config.toml");
    if !path.exists() {
        return Ok(Config::default());
    }

    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

fn build_config(args: Args, file_config: Config) -> Result<AppConfig, String> {
    let file_url = file_config
        .file_url
        .unwrap_or_else(|| "https://wetmore.ca/ip/haproxy_geo_ip.txt".to_string());
    let sha256_url = file_config
        .sha256_url
        .unwrap_or_else(|| "https://wetmore.ca/ip/haproxy_geo_ip.sha256".to_string());
    let local_file_path = file_config
        .local_file_path
        .unwrap_or_else(|| "haproxy_geo_ip.txt".to_string());
    let local_file_cidr = file_config
        .local_file_cidr
        .unwrap_or_else(|| "okcidr.txt".to_string());
    let asn_base_url = file_config
        .asn_base_url
        .unwrap_or_else(|| "https://raw.githubusercontent.com/ipverse/asn-ip/master/as".to_string());

    let mut country_codes = file_config.country_codes.unwrap_or_default();
    let mut asn_numbers = file_config.asn_numbers.unwrap_or_default();

    if !args.country_codes.is_empty() {
        country_codes = args.country_codes;
    }

    if !args.asn_numbers.is_empty() {
        asn_numbers = args.asn_numbers;
    }

    if country_codes.is_empty() {
        return Err("At least one country code must be provided via --country or config.toml".to_string());
    }

    let country_codes = country_codes
        .into_iter()
        .map(|cc| cc.to_uppercase())
        .collect();

    Ok(AppConfig {
        country_codes,
        asn_numbers,
        file_url,
        sha256_url,
        local_file_path,
        local_file_cidr,
        asn_base_url,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let file_config = load_config()?;
    let config = build_config(args, file_config).unwrap_or_else(|err| {
        eprintln!("{}", err);
        std::process::exit(1);
    });

    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();

    // Check for local file and get its modification time for an If-Modified-Since header
    if let Ok(metadata) = fs::metadata(&config.local_file_path) {
        if let Ok(modified_time) = metadata.modified() {
            let http_date = httpdate::fmt_http_date(modified_time);
            if let Ok(header_value) = HeaderValue::from_str(&http_date) {
                headers.insert("If-Modified-Since", header_value);
            }
        }
    }

    println!("Fetching IP geolocation data from: {}", config.file_url);
    let response = client
        .get(&config.file_url)
        .headers(headers)
        .send()
        .await?;

    let content = match response.status() {
        StatusCode::OK => {
            println!("New version of the file found, downloading...");
            let content = response.bytes().await?;

            // Verify SHA256 of the newly downloaded file
            println!("Verifying integrity with SHA256 from: {}", config.sha256_url);
            let sha256_response = client.get(&config.sha256_url).send().await?;
            let sha256_content = sha256_response.text().await?;
            let expected_hash = sha256_content.split_whitespace().next().unwrap_or("");

            let mut hasher = Sha256::new();
            hasher.update(&content);
            let calculated_hash = format!("{:x}", hasher.finalize());

            if calculated_hash != expected_hash {
                eprintln!("SHA256 mismatch! Downloaded file is corrupt.");
                eprintln!("Expected:   {}", expected_hash);
                eprintln!("Calculated: {}", calculated_hash);
                std::process::exit(1);
            }
            println!("SHA256 verification successful!");

            // Save the new content to the local file
            fs::write(&config.local_file_path, &content)?;
            println!("Local file updated.");
            content.to_vec()
        }
        StatusCode::NOT_MODIFIED => {
            println!("Local file is already up-to-date. Processing local file.");
            fs::read(&config.local_file_path)?
        }
        _ => {
            eprintln!("Failed to fetch file: {}", response.status());
            std::process::exit(1);
        }
    };

    // Process the content (either from download or local file)
    process_and_grep(&content, &config.country_codes, &config.local_file_cidr)?;

    // Process ASN data if any ASN numbers are provided
    if !config.asn_numbers.is_empty() {
        process_asn_data(
            &client,
            &config.asn_numbers,
            &config.asn_base_url,
            &config.local_file_cidr,
        )
        .await?;
    }

    Ok(())
}

fn process_and_grep(
    content: &[u8],
    country_codes: &[String],
    local_file_cidr: &str,
) -> io::Result<()> {
    let reader = BufReader::new(content);

    println!(
        "\nProcessing CIDR blocks for country codes: {:?}...",
        country_codes
    );

    let mut country_counts = std::collections::HashMap::new();
    let mut filtered_lines = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let columns: Vec<&str> = line.split_whitespace().collect();

        if columns.len() == 2 {
            let cidr_block = columns[0];
            let country_code = columns[1];

            if country_codes.iter().any(|cc| cc == country_code) {
                // Only store the CIDR block, not the country code
                filtered_lines.push(cidr_block.to_string());
                *country_counts.entry(country_code.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Write filtered results to local_file_cidr (CIDR blocks only)
    let output_content = filtered_lines.join("\n");
    fs::write(local_file_cidr, &output_content)?;

    println!("Filtered CIDR blocks written to: {}", local_file_cidr);
    println!("\nSummary:");

    let mut total = 0;
    for code in country_codes {
        let count = country_counts.get(code).unwrap_or(&0);
        println!("{} CIDR blocks: {}", code, count);
        total += count;
    }

    println!("Total matching blocks: {}", total);

    Ok(())
}

async fn process_asn_data(
    client: &reqwest::Client,
    asn_numbers: &[String],
    asn_base_url: &str,
    local_file_cidr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\nProcessing ASN data for: {:?}...", asn_numbers);

    let mut all_asn_blocks = Vec::new();
    let mut asn_counts = std::collections::HashMap::new();

    for asn in asn_numbers {
        let url = format!("{}/{}/ipv4-aggregated.txt", asn_base_url, asn);
        println!("Fetching ASN data from: {}", url);

        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let content = response.text().await?;
                    let lines: Vec<&str> = content.lines().collect();
                    let count = lines.len();

                    for line in lines {
                        let line = line.trim();
                        if !line.is_empty() {
                            // Only store the CIDR block, not the ASN suffix
                            all_asn_blocks.push(line.to_string());
                        }
                    }

                    asn_counts.insert(asn.clone(), count);
                    println!("AS{} CIDR blocks fetched: {}", asn, count);
                } else {
                    eprintln!(
                        "Warning: Failed to fetch AS{}: HTTP {}",
                        asn,
                        response.status()
                    );
                }
            }
            Err(e) => {
                eprintln!("Warning: Error fetching AS{}: {}", asn, e);
            }
        }
    }

    // Append ASN blocks to the existing okcidr.txt file
    if !all_asn_blocks.is_empty() {
        let mut existing_content =
            fs::read_to_string(local_file_cidr).unwrap_or_else(|_| String::new());

        if !existing_content.is_empty() && !existing_content.ends_with('\n') {
            existing_content.push('\n');
        }

        existing_content.push_str(&all_asn_blocks.join("\n"));
        fs::write(local_file_cidr, existing_content)?;

        println!("\nASN CIDR blocks appended to: {}", local_file_cidr);
        println!("\nASN Summary:");

        let mut total = 0;
        for asn in asn_numbers {
            let count = asn_counts.get(asn).unwrap_or(&0);
            println!("AS{} CIDR blocks: {}", asn, count);
            total += count;
        }
        println!("Total ASN blocks: {}", total);
    }

    Ok(())
}
