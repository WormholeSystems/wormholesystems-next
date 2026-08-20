# Deploying Vector

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

Caddy is the only thing published. `/api`, `/auth` and `/ws` go to the API; everything else
goes to the SvelteKit server. The database port is bound to the loopback so `psql` works
over SSH and nothing else can reach it.

## First run

```sh
git clone https://github.com/eve-vector/vector.git
cd vector
./vectorctl setup
```

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
./vectorctl update
```

Fast-forwards the checkout, rebuilds, restarts. Migrations run on boot, so there is no
separate step.

CCP's static data is deliberately not part of that. `status` says when a newer build is
out; taking it is another ~550MB download and a re-seed of a few minutes:

```sh
./vectorctl sde-update
```

## Checking on it

```sh
./vectorctl status
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
| `VECTOR_DOMAIN` | Caddy's site address. Blank serves plain http, which is only useful locally. |
| `HTTP_PORT`, `HTTPS_PORT` | What Caddy publishes. 80 and 443 in production. |
| `VECTOR_CONTACT_NAME`, `VECTOR_CONTACT_EMAIL` | Who runs this install. Every request to ESI, zKillboard and EVE Ref carries them, which is how those services tell operators apart and reach you. The server refuses to start without them. |
| `EVE_CLIENT_ID`, `EVE_CLIENT_SECRET` | The SSO application. |
| `EVE_REDIRECT_URI` | Derived from the domain; must match the application exactly. |
| `POSTGRES_PASSWORD` | Generated once. Changing it means changing it in Postgres too. |

Discord and the killmail ingest are optional and off unless configured; see `.env.example`.

## Discord

Optional, and `setup` offers it. Vector uses a Discord application for three things: linking
an account so `/vector` knows who you are, the slash commands themselves, and posting alerts
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
2. `./vectorctl discord-register` uploads the `/vector` command. It is registered globally,
   so Discord takes a few minutes to show it.

## Backups

Not automated. The database is the only thing that matters:

```sh
docker compose exec -T db pg_dump -U vector vector | gzip > vector-$(date +%F).sql.gz
```

Restoring is the same in reverse, into a stopped stack with an empty database.
