# Deployment

How the site actually runs in production. Superseded the old Docker /
ops-unikernel / certbot scaffolding that used to live in this file — none of
it reflected reality.

## Topology

```
                    Internet
                       │
        ┌──────────────┼──────────────┐
        │ 443 TCP      │ 80 TCP       │ 1965 TCP
        ▼              ▼              ▼
   ┌────────────────────────┐   ┌──────────────┐
   │     Caddy              │   │  homepage    │
   │  (TLS via ACME)        │   │  (Gemini)    │
   │  reverse_proxy ────────┼──▶│  0.0.0.0:1965│
   └────────────────────────┘   │              │
        ▲                       │              │
        │ HTTP 127.0.0.1:8080   │ HTTP 8080    │
        └───────────────────────│ 127.0.0.1    │
                                └──────────────┘
                              Hetzner VPS (palanthas)
                              aarch64, NixOS
```

- **HTTP/HTTPS**: Caddy terminates TLS on :443 (auto Let's Encrypt), reverse
  proxies to the Rust server on `127.0.0.1:8080`. Plain :80 redirects to :443.
- **Gemini**: the Rust server listens on `0.0.0.0:1965` directly with its own
  self-signed TLS certificate — no reverse proxy. This is the only
  process-owned socket reachable from the public internet.
- **Metrics**: WebSocket at `/__metrics__/ws` is public (no auth), routed
  through Caddy like any other HTTP path.

## Host

- **Provider**: Hetzner Cloud, aarch64
- **OS**: NixOS
- **SSH alias**: `palanthas` (configured in `~/.ssh/config`)
- **DNS**: `sven.guru` A/AAAA → VPS IP

## Split config — two repos

Unit file + user + Caddy vhost live in `~/nixos-config/`. The server binary
and the reference systemd unit live in this repo. Changes to the unit or
infra go via NixOS; changes to the binary go via scp.

| What | Where | Deploy with |
|------|-------|-------------|
| Rust binary (`/opt/homepage/static-server`) | this repo | `mise run deploy` |
| systemd unit, `homepage` user, Caddy vhost | `~/nixos-config/modules/homepage.nix` | `cd ~/nixos-config && mise run deploy:palanthas` |
| Persistent state dir `/var/lib/homepage/` | created by systemd from `StateDirectory=homepage` | — |

The reference `server/homepage.service` in this repo is **not** what systemd
loads on the VPS — it mirrors the NixOS unit so local dev / non-NixOS hosts
can reuse it. When in doubt, `~/nixos-config/modules/homepage.nix` wins.

## Binary deploy

From this repo:

```bash
mise run deploy
```

That runs `mise run build aarch64` then `server/deploy-vps.sh`:

1. Build Hugo site, convert Gemini content, cross-compile the static
   aarch64-musl binary (~5 MB).
2. `scp` to `/tmp/static-server.new` on the VPS.
3. `systemctl stop homepage`, move binary into `/opt/homepage/static-server`,
   chown `homepage:homepage`, `systemctl start homepage`.
4. Poll `curl http://localhost:8080/` for 30s; fail the deploy if it
   doesn't come up.
5. Verify `https://sven.guru/` returns 200.

No downtime-hiding tricks — restart is ~1 second. Good enough for a personal
site.

## NixOS / infra deploy

From `~/nixos-config/`:

```bash
mise run deploy:palanthas    # wraps: nix run github:serokell/deploy-rs -- -s .#palanthas
```

`deploy-rs` ships the closure, activates it, and rolls back if the
post-activation health check fails. The unit has
`ConditionPathExists=/opt/homepage/static-server` so a fresh server boots
fine before any binary has been deployed.

Deploy NixOS first when a change needs a new unit directive (new env var,
new capability, new sandbox setting); deploy the binary after.

## Bootstrapping a fresh host

1. Install NixOS on the VPS (one-time; Hetzner rescue + nixos-install).
2. Point DNS at the box.
3. `mise run deploy:palanthas` from `~/nixos-config/` — creates the
   `homepage` user, lays down the unit, brings up Caddy. Unit stays inactive
   because the binary isn't there yet (`ConditionPathExists`).
4. `mise run deploy` from this repo — ships the binary, starts the unit.
5. Caddy acquires the LE cert on first HTTPS request.

## Monitoring & troubleshooting

```bash
ssh palanthas

# Service state
systemctl status homepage
journalctl -u homepage -n 50 --no-pager

# Caddy
systemctl status caddy
journalctl -u caddy -n 50 --no-pager

# Sandbox score (expect 1.4 OK)
systemd-analyze security homepage

# Persistent state
ls -la /var/lib/homepage/
```

## Environment variables

Set by the NixOS unit:

- `PORT=8080` — HTTP listen port (localhost only)
- `DOMAIN=sven.guru` — used as the Gemini cert CN
- `STATE_DIRECTORY=/var/lib/homepage` — exported by systemd from
  `StateDirectory=homepage`; the binary looks here for
  `gemini.crt` / `gemini.key` and generates+persists them on first boot

Optional (not set in prod):

- `ENABLE_GEMINI` — `false` disables the Gemini listener (default: on if
  any Gemini content was compiled in)
- `DEBUG_GEMINI` — extra logging on dropped/timed-out Gemini connections

## Security

See [SECURITY_HARDENING.md](SECURITY_HARDENING.md) for the full audit: TLS
stack, Gemini listener timeouts/caps, WebSocket handshake validation,
systemd sandbox (exposure level 1.4), and the persistent-cert mechanism.
