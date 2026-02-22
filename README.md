# cidrfest

cidrfest builds a plain list of IPv4 CIDRs for the countries and ASNs you choose. It downloads a geolocation dataset and optional ASN CIDR lists, filters them, and writes a single output file.

## Highlights

- Filter by country codes and/or ASN numbers
- Case-insensitive country codes
- Caches the geolocation file with conditional HTTP download
- Verifies downloads with SHA256
- Outputs CIDRs only (one per line)

## Installation

1. Clone the repository:
```bash
git clone https://github.com/mortn/cidrfest.git
cd cidrfest
```

2. Build the application:
```bash
cargo build --release
```

## Usage

You must provide at least one country code (via CLI or config). ASN numbers are optional.

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

## Output

The generated `okcidr.txt` file contains one CIDR block per line:

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

**Note**: Use `src -f /etc/haproxy/okcidr.txt` NOT `src,map_ip()`. The `-f` flag is the correct way to match source IPs against a CIDR list file in HAProxy.

## Data Sources

- **Geolocation Data**: https://wetmore.ca/ip/haproxy_geo_ip.txt
- **SHA256 Checksum**: https://wetmore.ca/ip/haproxy_geo_ip.sha256
- **ASN Data**: https://github.com/ipverse/asn-ip (IPv4 aggregated CIDR blocks)

If you rely on this, please consider supporting the data providers.

## Command-Line Arguments

- `-c, --country <CODE>`: Country code to filter (required unless provided via `config.toml`, can be specified multiple times)
  - Example: `-c dk -c se -c no`
  - Case-insensitive

- `-a, --asn <NUMBER>`: ASN number to include (optional, can be specified multiple times)
  - Example: `-a 1234 -a 5678`

- `-h, --help`: Display help information

## License

MIT

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
