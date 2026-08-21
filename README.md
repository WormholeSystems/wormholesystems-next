# WormholeSystems

Collaborative wormhole mapping for EVE Online. One live chain that a corp shares: paste
your probe scanner and the signatures sync, mark what each hole will still take, see where
everyone is, and find the way home.

A Rust and SvelteKit rewrite of [wormhole.systems](https://wormhole.systems/).

> **Pre-alpha.** It runs, and it is being worked on daily. Things move, break and get
> rebuilt without warning, and the database is not yet treated as precious.

## Running it

You need Docker, a domain pointed at the machine, and an EVE application from the
[developer portal](https://developers.eveonline.com/) with the `esi-location.*` scopes.

```sh
curl -fsSL https://install.wormhole.systems | sh
wsctl setup
```

`wsctl setup` asks for what it needs, writes `.env`, and brings the stack up. See
[docs/deployment.md](./docs/deployment.md) for what it does and how to look after it
afterwards.

## Developing

Postgres, then the API and the frontend:

```sh
docker compose up -d db
cargo sqlx migrate run
cargo run -- seed          # the SDE: a few hundred MB the first time
cargo run                  # API on :3000
cd frontend && npm install && npm run dev
```

`cargo test` needs a database (`DATABASE_URL`); it builds its own per test. `cargo sqlx
prepare --workspace -- --all-targets` refreshes the offline query cache, which CI compiles
against, and a pre-push hook checks it is current.

## Layout

- `src/` — the API. `src/maps/` holds the rules, `src/api/` only decodes and dispatches.
- `frontend/` — SvelteKit. The map canvas, panels and their state.
- `migrations/` — the schema, each file creating its tables in final form.
- `wsctl/` — the operator CLI that installs, updates and inspects a deployment.
- `docs/` — how the thing works: data model, ESI endpoints, background processes.

## Licence

MIT. See [LICENSE](./LICENSE).
