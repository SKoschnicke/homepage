# Deployment Guide - Hetzner Cloud

Guide for deploying the Rust static server to Hetzner Cloud.

## Prerequisites

- Hetzner Cloud account with API token
- `hcloud` CLI installed: `brew install hcloud` or download from [Hetzner](https://github.com/hetznercloud/cli)
- Docker/Podman for container builds

## Option 1: Docker Deployment (Recommended)

### 1. Build the Docker image locally

```bash
cd /home/sven/development/homepage
podman build -f server/Dockerfile -t static-server:latest .
```

### 2. Create a Hetzner Cloud server

```bash
# Login to Hetzner
hcloud context create homepage

# Create a small server (CX22 = 2 vCPU, 4GB RAM, €5.83/month)
hcloud server create \
  --name homepage-server \
  --type cx22 \
  --image ubuntu-22.04 \
  --ssh-key <your-ssh-key-name> \
  --location fsn1
```

### 3. Install Docker on the server

```bash
# SSH into the server
ssh root@<server-ip>

# Install Docker
apt-get update
apt-get install -y docker.io

# Enable and start Docker
systemctl enable docker
systemctl start docker
```

### 4. Push and run the container

**Option A: Save/Load (no registry needed)**

```bash
# On your local machine - save the image
podman save static-server:latest | gzip > static-server.tar.gz

# Copy to server
scp static-server.tar.gz root@<server-ip>:/tmp/

# On the server - load and run
ssh root@<server-ip>
docker load < /tmp/static-server.tar.gz
docker run -d \
  --name homepage \
  -p 80:3000 \
  --restart unless-stopped \
  static-server:latest

# Check it's running
docker ps
curl http://localhost
```

**Option B: Use Docker Hub**

```bash
# Tag and push
podman tag static-server:latest your-dockerhub-username/static-server:latest
podman push your-dockerhub-username/static-server:latest

# On server
ssh root@<server-ip>
docker pull your-dockerhub-username/static-server:latest
docker run -d \
  --name homepage \
  -p 80:3000 \
  --restart unless-stopped \
  your-dockerhub-username/static-server:latest
```

### 5. Configure firewall

```bash
# On Hetzner Cloud Console or via CLI
hcloud firewall create --name web-firewall
hcloud firewall add-rule web-firewall \
  --direction in \
  --protocol tcp \
  --port 80 \
  --source-ips 0.0.0.0/0 \
  --source-ips ::/0

hcloud firewall add-rule web-firewall \
  --direction in \
  --protocol tcp \
  --port 443 \
  --source-ips 0.0.0.0/0 \
  --source-ips ::/0

hcloud firewall apply-to-resource web-firewall \
  --type server \
  --server homepage-server
```

### 6. Test the deployment

```bash
curl http://<server-ip>/
```

## Option 2: Simple Binary Deployment

For maximum performance without Docker overhead.

### 1. Create server (same as above)

### 2. Build a static binary

```bash
# On your local machine
cd /home/sven/development/homepage

# Build Hugo site
hugo --minify

# Build Rust binary (musl for static linking)
cd server
cargo build --release --target x86_64-unknown-linux-musl

# The binary is now at: target/x86_64-unknown-linux-musl/release/static-server
```

### 3. Copy to server

```bash
scp target/x86_64-unknown-linux-musl/release/static-server root@<server-ip>:/usr/local/bin/

# Make executable
ssh root@<server-ip> chmod +x /usr/local/bin/static-server
```

### 4. Create systemd service

```bash
# On the server
cat > /etc/systemd/system/homepage.service <<'EOF'
[Unit]
Description=Static Homepage Server
After=network.target

[Service]
Type=simple
User=www-data
WorkingDirectory=/var/www
ExecStart=/usr/local/bin/static-server
Restart=always
RestartSec=3
Environment="PORT=3000"

[Install]
WantedBy=multi-user.target
EOF

# Enable and start
systemctl daemon-reload
systemctl enable homepage
systemctl start homepage
systemctl status homepage
```

### 5. Setup nginx reverse proxy (optional, for HTTPS)

```bash
# Install nginx
apt-get install -y nginx certbot python3-certbot-nginx

# Configure nginx
cat > /etc/nginx/sites-available/homepage <<'EOF'
server {
    listen 80;
    server_name sven.guru www.sven.guru;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
EOF

ln -s /etc/nginx/sites-available/homepage /etc/nginx/sites-enabled/
nginx -t
systemctl reload nginx

# Get SSL certificate
certbot --nginx -d sven.guru -d www.sven.guru
```

## Option 3: True Unikernel Deployment

Hetzner doesn't natively support unikernels, but you can try custom images.

### 1. Build unikernel with ops

```bash
# Install ops
curl https://ops.city/get.sh -sSfL | sh

# Build unikernel
cd server
cargo build --release
ops build target/release/static-server -c config.json

# This creates a bootable image
```

### 2. Convert to qcow2 for Hetzner

```bash
# ops creates a raw image, convert it
qemu-img convert -f raw -O qcow2 static-server.img static-server.qcow2
```

### 3. Upload to Hetzner (complex)

Hetzner doesn't have a simple custom image upload API like AWS. You'd need to:

1. Create a snapshot server
2. Boot from rescue mode
3. Upload and write the image
4. Create snapshot
5. Use snapshot to boot new servers

**This is complex and not recommended for testing.** Use Docker or binary deployment instead.

## DNS Configuration

Point your domain to the server IP:

```bash
# Get server IP
hcloud server describe homepage-server

# Add A record to your DNS
# sven.guru → <server-ip>
```

## Updating the Deployment

### Docker:

```bash
# Rebuild image
podman build -f server/Dockerfile -t static-server:latest .

# Save and copy to server
podman save static-server:latest | gzip > static-server.tar.gz
scp static-server.tar.gz root@<server-ip>:/tmp/

# On server
ssh root@<server-ip>
docker load < /tmp/static-server.tar.gz
docker stop homepage
docker rm homepage
docker run -d --name homepage -p 80:3000 --restart unless-stopped static-server:latest
```

### Binary:

```bash
# Rebuild
cd /home/sven/development/homepage
hugo --minify
cd server
cargo build --release --target x86_64-unknown-linux-musl

# Deploy
scp target/x86_64-unknown-linux-musl/release/static-server root@<server-ip>:/usr/local/bin/
ssh root@<server-ip> systemctl restart homepage
```

## Monitoring

### Check server status

```bash
# Docker
ssh root@<server-ip> docker logs homepage

# Binary
ssh root@<server-ip> journalctl -u homepage -f
```

### Monitor performance

```bash
# Install monitoring tools on server
apt-get install -y htop iotop

# Watch resource usage
ssh root@<server-ip> htop

# Check connections
ss -tuln | grep 3000
```

## Cost Estimation

**Hetzner Cloud Pricing (as of 2024):**

- **CX11** (1 vCPU, 2GB RAM): €4.15/month - sufficient for low traffic
- **CX22** (2 vCPU, 4GB RAM): €5.83/month - recommended for testing
- **CX32** (4 vCPU, 8GB RAM): €11.49/month - high traffic

**Traffic:** 20TB included with all servers (more than enough for a personal site)

**Total estimated cost:** ~€6/month for testing

## Cleanup

```bash
# Delete the server when done testing
hcloud server delete homepage-server

# Or power off to save money
hcloud server poweroff homepage-server
```

## Recommended Approach for Testing

1. **Start simple:** Use Docker deployment (Option 1)
2. **Test it works:** Verify the site loads and performs well
3. **Add HTTPS:** Use nginx + certbot if you want SSL
4. **Optimize later:** Once satisfied, switch to binary deployment for max performance

The Docker approach is easiest to iterate on and gives you ~90% of the performance since the Rust server itself is still doing the heavy lifting.
