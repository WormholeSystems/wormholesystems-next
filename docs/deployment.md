# Deploying WormholeSystems

One machine, Docker, and a domain. Everything below assumes a fresh server you can point a
DNS record at.

## What runs

Four containers, from `docker-compose.yml` under the `full` profile:

| container | what it is | reachable from |
|---|---|---|
| `caddy` | TLS and the single public origin | the internet, ports 80 and 443 |
| `web` | the SvelteKit server (adapter-node) | Caddy only |
| `api` | the Axum API and every background job | Caddy only |
| `db` | Postgres 16 | the other containers, and `127.0.0.1` on the host |

Caddy is the only thing published. `/api`, `/auth`, `/ws` and `/discord` go to the API;
everything else goes to the SvelteKit server. The database port is bound to the loopback so
`psql` works over SSH and nothing else can reach it.

## Before you start

A server with Docker Engine and Compose v2, 5GB of disk, and a DNS A record pointing at it.
The release build wants about 2GB of memory: on a 2GB machine, add swap first or the
compiler is killed part way through.

The repository is private, so the server needs its own read-only access. Generate a key on
the server and add the public half as a deploy key under Settings → Deploy keys:

```sh
ssh-keygen -t ed25519 -C wormholesystems-deploy -f ~/.ssh/id_ed25519 -N ""
cat ~/.ssh/id_ed25519.pub
```

## First run

Install `wsctl`, the setup tool:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://install-next.wormhole.systems | sh
```

It works out this machine's platform, drops the binary in `/usr/local/bin`, and offers to
run the setup straight away. Pin a version with `WSCTL_VERSION=wsctl-v0.1.0`.

Then, in a checkout:

```sh
git clone git@github.com:WormholeSystems/wormholesystems-next.git
cd wormholesystems-next
wsctl setup
```

While the repository is private, that download needs a token that can read it:

```sh
export WSCTL_TOKEN=ghp_...
curl -fsSL -H "Authorization: Bearer $WSCTL_TOKEN" \
  https://raw.githubusercontent.com/WormholeSystems/wormholesystems-next/main/install.sh | sh
```

A private repository's release assets are not reachable by plain URL at all, so with a
token the installer goes through the API instead: it finds the asset by name and asks for
its bytes.

### Building it instead

No release for your platform, no token, or you would rather not download a binary. The
checkout has the source, and Docker is already here:

```sh
docker run --rm -u "$(id -u):$(id -g)" -e CARGO_HOME=/tmp/cargo \
  -v "$PWD":/w -w /w rust:1.94-slim-bookworm cargo build --release -p wsctl
sudo install -m755 target/release/wsctl /usr/local/bin/wsctl
```

Running as yourself rather than root keeps `target/` yours; without `-u` the build leaves
root-owned files behind. With Rust already installed, `cargo run -p wsctl -- setup` does
the same thing without the container.

It checks the machine before asking for anything: Docker present and running, compose v2,
disk space, and whether this checkout is behind origin. Then it asks for the domain and the
EVE SSO credentials, checks that the domain resolves to this machine and that ports 80 and
443 are free, generates a database password, writes `.env` with `chmod 600`, and brings the
stack up.

The EVE application's callback URL has to match exactly what the CLI prints:
`https://your-domain/auth/callback`. Register it at
<https://developers.eveonline.com/applications>.

The first boot downloads CCP's static data export (~550MB) and seeds from it, which takes
a few minutes. It lands in a volume, so later restarts and rebuilds reuse it. Watch it with:

```sh
docker compose --profile full logs -f api
```

## Updating

```sh
wsctl update
```

Fast-forwards the checkout, rebuilds, restarts, and takes CCP's static data when a newer
build is out. Migrations run on boot, so there is no separate step for those either.

The static data is the slow half: another ~550MB and a re-seed of a few minutes. It is
skipped when the loaded build is already current, and `--sde` fetches it regardless, which
is what to reach for if a download was interrupted:

```sh
wsctl update --sde
```

## Checking on it

```sh
wsctl status
```

Containers, how far behind origin the checkout is, which SDE build is loaded and whether
CCP has a newer one, and whether the public URL answers.

## Certificates

Caddy gets them from Let's Encrypt on first start and renews them on its own. Two things
have to hold or issuance fails: the domain resolves to this machine, and port 80 reaches
Caddy from the internet. The certificates live in the `caddy_data` volume; keep it. Losing
it means re-issuing, and Let's Encrypt rate-limits that.

Behind Cloudflare's proxy, DNS resolves to Cloudflare rather than the server, so `setup`
warns rather than refuses. Either turn the proxy off while the certificate is issued, or use
Cloudflare's own certificate and put Caddy behind it.

## What is in `.env`

`setup` writes these; the rest of the file is the same as `.env.example`.

| key | why |
|---|---|
| `WS_DOMAIN` | Caddy's site address. Blank serves plain http, which is only useful locally. |
| `HTTP_PORT`, `HTTPS_PORT` | What Caddy publishes. 80 and 443 in production. |
| `WS_CONTACT_NAME`, `WS_CONTACT_EMAIL` | Who runs this install. Every request to ESI, zKillboard and EVE Ref carries them, which is how those services tell operators apart and reach you. The server refuses to start without them. |
| `EVE_CLIENT_ID`, `EVE_CLIENT_SECRET` | The SSO application. |
| `EVE_REDIRECT_URI` | Derived from the domain; must match the application exactly. |
| `POSTGRES_PASSWORD` | Generated once. Changing it means changing it in Postgres too. |

Discord and the killmail ingest are optional and off unless configured; see `.env.example`.

## Discord

Optional, and `setup` offers it. WormholeSystems uses a Discord application for three things: linking
an account so `/wh` knows who you are, the slash commands themselves, and posting alerts
to a channel. Alerts to a webhook work without any of this.

At <https://discord.com/developers/applications>, create an application and take:

| where | what | into |
|---|---|---|
| General Information | Application ID | `DISCORD_APPLICATION_ID` |
| General Information | Public Key | `DISCORD_PUBLIC_KEY` |
| OAuth2 | Client ID and Client Secret | `DISCORD_CLIENT_ID`, `DISCORD_CLIENT_SECRET` |
| Bot | Token, only to post as the bot or send DMs | `DISCORD_BOT_TOKEN` |

Add `https://your-domain/discord/callback` as an OAuth2 redirect, exactly.

Two things have to wait until the stack is running, because Discord checks them:

1. Set the Interactions Endpoint URL to `https://your-domain/discord/interactions`. Discord
   signs a ping at it and refuses to save if it does not answer.
2. `wsctl discord-register` uploads the `/wh` command. It is registered globally,
   so Discord takes a few minutes to show it.

## Backups

Not automated. The database is the only thing that matters:

```sh
docker compose exec -T db pg_dump -U vector vector | gzip > wormholesystems-$(date +%F).sql.gz
```

Restoring is the same in reverse, into a stopped stack with an empty database.
