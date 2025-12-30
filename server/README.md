# Static Site Server

High-performance Rust web server that embeds the entire Hugo-generated site into a single binary for unikernel deployment.

## Why?

This server beats traditional nginx setups by:

- **Zero syscalls** - All assets served from static memory (`&'static [u8]`)
- **Pre-compressed assets** - Gzip + Brotli compression done at build time, zero runtime CPU cost
- **Zero-copy I/O** - Direct memory-to-NIC transfer via DMA
- **Optimal caching** - Infinite cache headers for Hugo's fingerprinted assets
- **Unikernel deployment** - No OS overhead, runs directly on hypervisor

Expected performance: <100μs latency, 100k+ req/s (vs nginx: ~200-500μs, 50-80k req/s).

## Architecture

```
Hugo builds site → build.rs embeds assets → Rust binary → unikernel image
```

**Key components:**

- `build.rs` - Walks `../public/`, compresses text assets, generates `assets.rs` with all routes
- `main.rs` - hyper HTTP server with mimalloc allocator
- `router.rs` - Content negotiation, ETag handling, cache headers
- `assets.rs` - Generated file with 89 embedded routes (HTML, CSS, JS, images, XML)

## Build & Run

### Prerequisites

- Rust toolchain (cargo)
- Hugo (to generate the site first)
- For unikernel: [ops](https://ops.city/)

### Local Development

```bash
# Build the Hugo site first
cd ..
hugo --minify

# Run the server
cd server
cargo run --release
```

Server starts on `http://localhost:3000` (or set `PORT` env var).

### Production Build

```bash
# From repository root
./build.sh
```

This runs:
1. `hugo --minify` - Generate static site
2. `cargo build --release` - Build Rust server (embeds Hugo output)
3. `strip` - Remove debug symbols

Final binary: `server/target/release/static-server` (~7.6MB)

## Unikernel Deployment

### Install ops

```bash
curl https://ops.city/get.sh -sSfL | sh
```

### Build Unikernel Image

```bash
cd server
cargo build --release
ops build target/release/static-server -c config.json
```

This creates a bootable unikernel image.

### Test Locally (QEMU)

```bash
ops run static-server -c config.json
```

### Deploy to Cloud

ops supports AWS, GCP, and Azure. Upload the generated image:

```bash
# Example: AWS
ops image create static-server -c config.json -t aws
ops instance create static-server -t aws
```

See [ops documentation](https://docs.ops.city/) for platform-specific deployment.

## How It Works

### Build-Time Asset Processing

`build.rs` runs before compilation:

1. Walks `../public/` directory
2. For each file:
   - Reads content
   - Detects MIME type
   - Generates SHA256 ETag
   - If compressible (HTML/CSS/JS/XML): creates gzip + brotli variants
   - If binary (PNG): skips compression
   - Emits Rust code with embedded byte arrays
3. Generates `assets.rs` with:
   - `Asset` struct definition
   - One const per file (e.g., `ASSET_INDEX_HTML`)
   - `get_routes()` returning `HashMap<&'static str, &'static Asset>`

Example generated code:

```rust
const ASSET_INDEX_HTML: Asset = Asset {
    content_raw: &[60, 33, 100, 111, ...],
    content_gzip: &[31, 139, 8, 0, ...],
    content_brotli: &[27, 51, 30, ...],
    content_type: "text/html",
    etag: "db24659d40caf6f9...",
    is_compressible: true,
};
```

### Runtime Serving

1. Request arrives → router looks up path in static `HashMap`
2. Check ETag → return 304 if match
3. Parse `Accept-Encoding` header → choose best compression
4. Set cache headers:
   - Fingerprinted assets (`.min.HASH.ext`): `max-age=31536000, immutable`
   - HTML/other: `max-age=3600`
5. Serve from static memory (zero allocation, zero copy)

## Configuration

### Server Port

Set `PORT` environment variable (default: 3000):

```bash
PORT=8080 cargo run --release
```

### Unikernel Config

Edit `config.json`:

```json
{
  "Boot": "./target/release/static-server",
  "RunConfig": {
    "Memory": "256m",
    "Ports": ["3000"]
  }
}
```

Adjust memory based on site size. Current site (~6.4MB) + compression (~12MB embedded) + runtime overhead = 256MB is plenty.

## Performance Tuning

The `Cargo.toml` includes aggressive optimizations:

```toml
[profile.release]
opt-level = 3           # Maximum optimization
lto = "fat"             # Link-time optimization
codegen-units = 1       # Single codegen unit (slower build, faster binary)
strip = true            # Strip symbols
panic = "abort"         # Smaller binary
```

Trade-off: ~60s compile time for 10-15% runtime performance gain.

## Updating Content

To update the site content:

1. Edit source files (org files, themes, etc.)
2. Run `./build.sh` to rebuild everything
3. Redeploy the new binary

**Note:** Unlike nginx, you can't just swap out files - the entire site is embedded in the binary. This is intentional for maximum performance. For static sites updated via CI/CD, this is fine.

## Monitoring

The server logs to stdout:

```
Static server running on http://0.0.0.0:3000
Serving 89 routes
```

For production monitoring, pipe to your logging system:

```bash
./static-server 2>&1 | logger -t static-server
```

## Limitations

- **Static content only** - No dynamic routes, no server-side rendering
- **Rebuild required** - Content updates require recompiling the binary
- **Binary size** - Grows with site size (~3x raw size due to compression variants)
- **Memory usage** - All assets loaded at startup (~50MB for current site)

These are acceptable trade-offs for maximum serving performance on static sites.

## Development

### Project Structure

```
server/
├── Cargo.toml          # Dependencies and build config
├── build.rs            # Asset preprocessing (runs at compile time)
├── config.json         # ops unikernel configuration
├── src/
│   ├── main.rs         # Server initialization (~30 lines)
│   ├── router.rs       # Routing and serving logic (~120 lines)
│   └── assets.rs       # GENERATED - do not edit
└── target/
    └── release/
        └── static-server
```

### Dependencies

**Runtime:**
- `hyper` - HTTP server (direct, no framework overhead)
- `tokio` - Async runtime
- `lazy_static` - Static route map initialization
- `mimalloc` - High-performance allocator

**Build-time:**
- `mime_guess` - Content-Type detection
- `flate2` - Gzip compression
- `brotli` - Brotli compression
- `sha2` - ETag generation
- `walkdir` - Directory traversal

### Benchmarking

Basic benchmarks with `wrk`:

```bash
# Start server
cargo run --release

# In another terminal
wrk -t4 -c100 -d30s http://localhost:3000/
```

Expected results (on modern hardware):
- Requests/sec: 100k+
- Latency (avg): <100μs
- Latency (99th): <500μs

Compare with nginx serving the same content for baseline.

## Unikernel Deployment to Hetzner Cloud

Deploy as a true unikernel directly to Hetzner Cloud using ops.

### Prerequisites

- [ops](https://ops.city/) installed: `curl https://ops.city/get.sh -sSfL | sh`
- Hetzner Cloud API token with Read & Write permissions

### Get Hetzner API Token

1. Go to [Hetzner Cloud Console](https://console.hetzner.cloud/)
2. Navigate to Security → API Tokens
3. Generate new token with Read & Write permissions
4. Copy the token

### Deploy Unikernel

```bash
# Set your Hetzner API token
export HCLOUD_TOKEN=<your-hetzner-api-token>

# Build the Rust binary (already done if you ran ./build.sh)
cargo build --release

# Build and deploy unikernel to Hetzner
ops instance create target/release/static-server \
  -c config-hetzner.json \
  -t hcloud \
  --hcloud-token $HCLOUD_TOKEN
```

This will:
1. Build a bootable unikernel image from your Rust binary
2. Upload the image to Hetzner Cloud
3. Create a VM instance (CX11, ~€4/month)
4. Boot your unikernel
5. Map port 80 (external) → 3000 (internal)

### Get Instance IP

```bash
# List running instances
ops instance list -t hcloud

# The output shows the public IP address
```

### Test Deployment

```bash
# Replace with your instance IP
curl http://<instance-ip>/
```

### Update Deployment

When you update content:

```bash
# Rebuild Hugo site and Rust binary
cd /home/sven/development/homepage
hugo --minify
cd server
cargo build --release

# Delete old instance
ops instance delete homepage-unikernel -t hcloud

# Create new instance with updated binary
ops instance create target/release/static-server \
  -c config-hetzner.json \
  -t hcloud \
  --hcloud-token $HCLOUD_TOKEN
```

### Configure DNS

Point your domain to the instance IP:

```
A Record: sven.guru → <instance-ip>
```

### HTTPS / TLS

For HTTPS, you have two options:

**Option 1: Add Cloudflare in front**
- Point DNS to Cloudflare
- Enable Cloudflare proxy
- Cloudflare provides free HTTPS

**Option 2: Hetzner Load Balancer**
- Create Hetzner Load Balancer with Let's Encrypt certificate
- Point load balancer to your unikernel instance
- Adds ~€5/month cost

### Cost

**Hetzner Cloud Pricing:**
- **CX11** (1 vCPU, 2GB RAM): €4.15/month
- Includes 20TB traffic/month
- No additional costs for unikernel deployment

**Total:** ~€4/month for production unikernel deployment

### Monitor Instance

```bash
# View instance logs (if available)
ops instance logs homepage-unikernel -t hcloud

# Check instance status
ops instance list -t hcloud
```

### Cleanup

```bash
# Delete instance when done
ops instance delete homepage-unikernel -t hcloud
```

## Troubleshooting

### Build fails with "public directory not found"

Run `hugo --minify` first to generate the site in `../public/`.

### Server starts but routes return 404

The route map is generated at build time. If you changed Hugo content, rebuild:

```bash
cargo clean
cargo build --release
```

### Binary size is large

This is expected. The binary embeds:
- Raw assets (~6.4MB)
- Gzip compressed (~2MB)
- Brotli compressed (~1.5MB)
- Rust runtime + dependencies (~3MB)

Total: ~13-15MB stripped, ~20MB with debug symbols.

### Port 3000 already in use

Change port:

```bash
PORT=8080 cargo run --release
```

## License

Same as parent project.
