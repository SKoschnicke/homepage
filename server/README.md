# Static Site Server

High-performance Rust web server that embeds the entire Hugo-generated site into a single binary.

## Why?

- **Zero syscalls** - All assets served from static memory (`&'static [u8]`)
- **Pre-compressed assets** - Gzip + Brotli compression done at build time, zero runtime CPU cost
- **Optimal caching** - Infinite cache headers for Hugo's fingerprinted assets
- **Tiny binary** - ~4MB statically linked, runs anywhere

## Architecture

```
Hugo builds site → build.rs embeds assets → Rust binary → deploy via scp
```

**Key components:**

- `build.rs` - Walks `../public/`, compresses text assets, generates `assets.rs` with all routes
- `main.rs` - HTTP server on localhost, Gemini server with self-signed TLS
- `router.rs` - Content negotiation, ETag handling, cache headers
- `assets.rs` - Generated file with embedded routes (HTML, CSS, JS, images, XML)
- `acme.rs` - Self-signed certificate generation (for Gemini only)
- `gemini.rs` - Gemini protocol handler
- `metrics.rs` - Real-time metrics collection and WebSocket streaming
- `websocket.rs` - WebSocket protocol handling for live metrics

## Build & Run

### Prerequisites

Nix flake provides all dependencies. Just enter the dev shell:

```bash
cd /path/to/homepage
# direnv does this automatically, or:
nix develop
```

### Local Development

```bash
mise run dev
```

Builds the site and starts the server on `http://localhost:8080`.

### Production Build (cross-compile for aarch64)

```bash
mise run build aarch64
```

This cross-compiles a statically-linked musl binary for ARM64. Output: `server/target/aarch64-unknown-linux-musl/release/static-server`.

### Native Build

```bash
mise run build
```

Output: `server/target/release/static-server`.

## Deployment

The site runs on a **Hetzner VPS (aarch64)** behind **Caddy** as reverse proxy. Caddy handles TLS via Let's Encrypt. The Rust server only listens on localhost.

### Deploy

```bash
mise run deploy
```

This will:
1. Build Hugo site + Gemini content
2. Cross-compile the Rust binary for aarch64-musl (static)
3. `scp` to the VPS (`palanthas`)
4. Restart the systemd service
5. Verify the site is responding

### Infrastructure

The VPS is managed via NixOS (`~/nixos-config/`):
- `modules/homepage.nix` - Systemd service, user, Caddy virtualhost
- `hosts/palanthas/default.nix` - Enables Caddy + homepage service

Deploy NixOS config changes separately:
```bash
cd ~/nixos-config
mise run deploy:palanthas
```

### Environment Variables

- `PORT` - HTTP listen port (default: 8080)
- `DOMAIN` - Domain name, used for Gemini self-signed cert (default: localhost)
- `ENABLE_GEMINI` - Enable Gemini server on port 1965 (default: true)

## How It Works

### Build-Time Asset Processing

`build.rs` runs before compilation:

1. Walks `../public/` directory
2. For each file:
   - Reads content, detects MIME type, generates SHA256 ETag
   - If compressible (HTML/CSS/JS/XML): creates gzip + brotli variants
   - If binary (PNG): skips compression
   - Emits Rust code with embedded byte arrays
3. Generates `assets.rs` with a static route map

### Runtime Serving

1. Request arrives → router looks up path in static `HashMap`
2. Check ETag → return 304 if match
3. Parse `Accept-Encoding` header → choose best compression
4. Set cache headers:
   - Fingerprinted assets (`.min.HASH.ext`): `max-age=31536000, immutable`
   - HTML/other: `max-age=3600`
5. Serve from static memory (zero allocation, zero copy)

### Gemini Protocol

The server also speaks Gemini (port 1965) with a self-signed TLS certificate. Gemini content is generated from the Hugo site by `scripts/convert-gemini-content.sh` using Pandoc.

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

## Project Structure

```
server/
├── Cargo.toml          # Dependencies and build config
├── build.rs            # Asset preprocessing (runs at compile time)
├── deploy-vps.sh       # VPS deployment script (called by mise)
├── homepage.service    # Systemd unit reference
├── src/
│   ├── main.rs         # Server initialization
│   ├── router.rs       # HTTP routing and serving
│   ├── acme.rs         # Self-signed cert generation (Gemini)
│   ├── gemini.rs       # Gemini protocol handler
│   ├── metrics.rs      # Request metrics
│   ├── websocket.rs    # WebSocket for metrics
│   └── assets.rs       # GENERATED - do not edit
└── target/
    └── aarch64-unknown-linux-musl/
        └── release/
            └── static-server
```

## Dependencies

**Runtime:**
- `hyper` - HTTP server (direct, no framework overhead)
- `tokio` - Async runtime
- `lazy_static` - Static route map initialization
- `mimalloc` - High-performance allocator
- `rustls` / `tokio-rustls` - TLS for Gemini
- `rcgen` - Self-signed certificate generation
- `tokio-tungstenite` - WebSocket for metrics
- `parking_lot` - High-performance locks

**Build-time:**
- `mime_guess` - Content-Type detection
- `flate2` - Gzip compression
- `brotli` - Brotli compression
- `sha2` - ETag generation
- `walkdir` - Directory traversal

## Troubleshooting

### Build fails with "public directory not found"

Run `hugo --minify` first to generate the site in `../public/`.

### Server starts but routes return 404

The route map is generated at build time. Rebuild after content changes:

```bash
cargo clean && cargo build --release
```

### Cross-compilation fails with "can't find crate for std"

Make sure direnv has loaded the flake (check for `impure (nix-shell-env)` in your prompt). The flake provides the aarch64-musl Rust target via fenix.

## License

Same as parent project.
