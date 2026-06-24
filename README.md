# homelab_api

Rust API for the [homelab](https://github.com/andrii-su/homelab). Serves service
data to the iOS Swift app ([homelab_app](https://github.com/andrii-su/homelab_app))
and controls Docker containers — replacing the current scraping of the Homepage
dashboard (`:3002`) with a typed, authenticated API.

**Stack:** Axum · Tokio · [bollard](https://crates.io/crates/bollard) (Docker API) · reqwest

> ⚠️ This API can start/stop containers on the host. It is **public code but not
> a public service** — run it behind the gateway + bearer token + Tailscale, and
> never expose port 8087 to the internet.

## Endpoints

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| GET  | `/health` | — | Liveness + Docker reachability |
| GET  | `/api/services` | ✅ | List containers (name, stack, state, status) |
| POST | `/api/services/:name/start` | ✅ | Start a container |
| POST | `/api/services/:name/stop` | ✅ | Stop a container |
| POST | `/api/services/:name/restart` | ✅ | Restart a container |
| GET  | `/api/services/:name/logs` | ✅ | Last 200 log lines |
| GET  | `/api/services/:name/stats` | ✅ | One-shot CPU/memory snapshot |
| POST | `/api/notify` | ✅ | Push relay → forwards to `WEBHOOK_URL` |

Auth = `Authorization: Bearer <API_TOKEN>`.

### Push relay

Container hooks / monitoring alerts `POST /api/notify`:

```json
{ "title": "Pi-hole down", "message": "DNS unreachable", "priority": "high", "tags": ["pihole"] }
```

The API forwards the JSON to `WEBHOOK_URL` (ntfy, Slack-compatible, or a custom
APNs proxy for native iOS push — swap the target without touching the app). If
`WEBHOOK_URL` is unset, events are logged so the pipeline works during setup.

## Run

```bash
cp .env.example .env
# set API_TOKEN, e.g.: openssl rand -hex 32
cargo run                      # needs the Docker socket reachable

# or containerized as a homelab stack:
docker compose up -d --build
```

Requires Rust stable (`rustup`) and a reachable Docker daemon
(`DOCKER_HOST` or `/var/run/docker.sock`).

## Deploy as a homelab stack

Copy this repo (or just `docker-compose.yml`) into `~/homelab/stacks/api/`,
add `API_TOKEN` to `~/homelab/.env`, then `docker compose up -d --build`.
Add a Caddy route in the gateway for `api.lab.home.arpa`.

## Roadmap

- [ ] Live log/stat streaming over SSE or WebSocket
- [ ] Host metrics (CPU/mem/disk/uptime) for the app dashboard
- [ ] `docker-socket-proxy` to restrict the API to a verb allowlist
- [ ] Stack-level actions (`make <stack>-up`) instead of per-container only
- [ ] Native APNs push module (replace generic relay)
- [ ] Published image to GHCR via CI

## License

MIT
