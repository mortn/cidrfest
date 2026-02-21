# HAProxy Geo & ASN ACL Generator

## What is this?
This is a rust command-line application that fetches IP geolocation data and ASN CIDR blocks and generates HAProxy-compatible ACL files filtered by country codes and/or ASN numbers.

## Features

- **Flexible Filtering**: Filter CIDR blocks by country code(s) and/or ASN number(s)
- **Efficient Caching**: Only downloads the geolocation file if it's newer than the local cached version using HTTP `If-Modified-Since` headers
- **Integrity Verification**: Validates downloaded geolocation files using SHA256 checksums
- **ASN Support**: Fetch and include CIDR blocks for specific Autonomous System Numbers (ASNs)
- **HAProxy Ready**: Generates output file (`okcidr.txt`) with CIDR blocks only, compatible with HAProxy's ACL `-f` flag
- **Case-Insensitive**: Country codes work in any case (dk, DK, Dk all work the same)
- **Error Handling**: Robust error handling for network issues and file corruption

## How It Works

1. **Local File Check**: Checks if a local copy of `haproxy_geo_ip.txt` exists and gets its modification time
2. **Conditional Download**: Sends an HTTP request with `If-Modified-Since` header to only download if the remote file is newer
3. **Integrity Verification**: Downloads and verifies the SHA256 checksum for any newly fetched geolocation files
4. **Country Filtering**: Parses the file to extract CIDR blocks matching the specified country codes
5. **ASN Fetching**: Downloads CIDR blocks for specified ASN numbers from the ipverse/asn-ip repository
6. **Output Generation**: Writes all CIDR blocks (country + ASN) to `okcidr.txt` in HAProxy-compatible format

## Prerequisites

- Rust 1.70+ (2021 edition)
- Internet connection for downloading geolocation and ASN data

## Installation

1. Clone the repository:
```bash
git clone https://github.com/mortn/ha-geo-asn-acl.git
cd ha-geo-asn-acl
```

2. Build the application:
```bash
cargo build --release
```

## Usage

The application requires at least one country code and supports optional ASN numbers.

### Config File (config.toml)

The application will read an optional `config.toml` file from the current working directory. CLI arguments override values in the config file.

Precedence order:

1. Hardcoded defaults
2. `config.toml`
3. CLI arguments

Example `config.toml`:

```toml
country_codes = ["DK", "SE"]
asn_numbers = ["1234"]

file_url = "https://wetmore.ca/ip/haproxy_geo_ip.txt"
sha256_url = "https://wetmore.ca/ip/haproxy_geo_ip.sha256"
local_file_path = "haproxy_geo_ip.txt"
local_file_cidr = "okcidr.txt"
asn_base_url = "https://raw.githubusercontent.com/ipverse/asn-ip/master/as"
```

If `country_codes` is not provided in `config.toml`, you must pass at least one `--country` on the CLI.

### Basic Usage - Country Codes Only

Filter CIDR blocks for one or more countries:

```bash
# Single country
cargo run -- -c dk

# Multiple countries
cargo run -- -c dk -c se -c no

# Using long form
cargo run -- --country dk --country se

# Case insensitive
cargo run -- -c DK -c Se -c NO
```

### Advanced Usage - Country Codes + ASN Numbers

Include CIDR blocks from specific Autonomous System Numbers:

```bash
# Countries with one ASN
cargo run -- -c dk -c se -a 1234

# Countries with multiple ASNs
cargo run -- -c dk -c se -a 1234 -a 5678 -a 9012

# Using long form
cargo run -- --country dk --asn 1234 --asn 5678
```

### Running the Compiled Binary

```bash
./target/release/cidrfest -c dk -c se -a 1234
```

## Sample Output

```
Fetching IP geolocation data from: https://wetmore.ca/ip/haproxy_geo_ip.txt
New version of the file found, downloading...
Verifying integrity with SHA256 from: https://wetmore.ca/ip/haproxy_geo_ip.sha256
SHA256 verification successful!
Local file updated.

Processing CIDR blocks for country codes: ["DK", "SE"]...
Filtered CIDR blocks written to: okcidr.txt

Summary:
DK CIDR blocks: 245
SE CIDR blocks: 312
Total matching blocks: 557

Processing ASN data for: ["1234"]...
Fetching ASN data from: https://raw.githubusercontent.com/ipverse/asn-ip/master/as/1234/ipv4-aggregated.txt
AS1234 CIDR blocks fetched: 128

ASN CIDR blocks appended to: okcidr.txt

ASN Summary:
AS1234 CIDR blocks: 128
Total ASN blocks: 128
```

## Output File Format

The generated `okcidr.txt` file contains one CIDR block per line, ready for use with HAProxy:

```
5.44.64.0/19
5.45.96.0/19
5.103.128.0/19
192.0.2.0/24
203.0.113.0/24
```

## HAProxy Integration

Use the generated `okcidr.txt` file in your HAProxy configuration:

```haproxy
frontend http-in
    bind *:80
    
    # Define ACL using the generated CIDR list
    acl acl_cidr_ok src -f /etc/haproxy/okcidr.txt
    
    # Allow only IPs in the list
    http-request deny unless acl_cidr_ok
    
    default_backend servers

backend servers
    server server1 192.168.1.10:8080
```

**Important**: Use `src -f /etc/haproxy/okcidr.txt` NOT `src,map_ip()`. The `-f` flag is the correct way to match source IPs against a CIDR list file in HAProxy.

## Data Sources

- **Geolocation Data**: https://wetmore.ca/ip/haproxy_geo_ip.txt
- **SHA256 Checksum**: https://wetmore.ca/ip/haproxy_geo_ip.sha256
- **ASN Data**: https://github.com/ipverse/asn-ip (IPv4 aggregated CIDR blocks)

The geolocation data file contains two columns:
1. **CIDR Block**: IP address range in CIDR notation
2. **Country Code**: Two-letter ISO country code

## File Structure

```
.
├── Cargo.toml
├── src/
│   └── main.rs
├── haproxy_geo_ip.txt    # Cached geolocation data (created after first run)
├── okcidr.txt            # Generated HAProxy ACL file
└── README.md
```

## Command-Line Arguments

- `-c, --country <CODE>`: Country code to filter (required unless provided via `config.toml`, can be specified multiple times)
  - Example: `-c dk -c se -c no`
  - Case-insensitive

- `-a, --asn <NUMBER>`: ASN number to include (optional, can be specified multiple times)
  - Example: `-a 1234 -a 5678`

- `-h, --help`: Display help information

## Dependencies

- **reqwest**: HTTP client for fetching remote files
- **sha2**: SHA-256 hashing for integrity verification
- **tokio**: Async runtime for efficient I/O operations
- **httpdate**: HTTP date parsing for conditional requests
- **clap**: Command-line argument parsing

## Error Handling

The application handles several error conditions:

- **Network failures**: Connection timeouts, DNS resolution issues
- **HTTP errors**: 404, 500, and other HTTP status codes (with warnings for ASN fetches)
- **Checksum mismatches**: Corrupted downloads are detected and rejected
- **File I/O errors**: Permission issues, disk space problems
- **Missing ASN data**: Continues processing other ASNs if one fails

## Performance

- **Bandwidth Efficient**: Only downloads geolocation data when files are updated
- **Fast Processing**: Async I/O for network operations
- **Memory Efficient**: Streams file processing without loading entire file into memory
- **Parallel ASN Fetching**: Fetches multiple ASN files efficiently

## Use Cases

- **Geo-blocking**: Restrict access to specific countries
- **Geo-allowing**: Only allow access from specific countries
- **ASN filtering**: Block or allow traffic from specific network providers
- **Mixed filtering**: Combine country and ASN filters for fine-grained control
- **HAProxy ACLs**: Generate ready-to-use ACL files for HAProxy

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## ToDo
- Add systemd files to demonstrate how to let this app run as non-root, and still allow us to smoothly copy an updated okcidr.txt file to /etc/haproxy/okcidr.txt and then reload the haproxy

## systemd

Example units to run cidrfest once per day from `/etc/cidrfest`:

```bash
sudo install -m 0755 /usr/local/bin/cidrfest /usr/local/bin/cidrfest
sudo install -d /etc/cidrfest
sudo install -m 0644 cidrfest.service /etc/systemd/system/cidrfest.service
sudo install -m 0644 cidrfest.timer /etc/systemd/system/cidrfest.timer

sudo systemctl daemon-reload
sudo systemctl enable --now cidrfest.timer

# run once immediately
sudo systemctl start cidrfest.service
```

## Changelog

### v0.2.0
- Added command-line argument support with clap
- Added ASN filtering support
- Case-insensitive country code matching
- HAProxy-compatible output (CIDR blocks only, no labels)
- Support for multiple countries and ASNs
- Changed output file to `okcidr.txt`

### v0.1.0
- Initial release
- Basic file fetching and SHA256 verification
- Country code filtering for DK and SE
- Conditional downloading with caching
