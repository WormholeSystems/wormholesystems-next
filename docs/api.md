# The API for scripts

Everything the map screen does goes over the JSON API under `/api/`, and all of it accepts
a personal access token in place of the session cookie. On top of that, `/api/v1/` serves
a small, stable surface in the shape the legacy application served, so a script written
against that can be pointed here.

## Authentication

A user mints a token under **Settings, API tokens**. The plaintext is shown once; only its
SHA-256 hash is stored ([`personal_access_tokens`](./database/authentication.md#personal_access_tokens)).
Send it as a bearer header:

```
Authorization: Bearer wst_...
```

A token acts as the user who made it, flying their preferred character, with exactly the
access that user has signed in: the same roles, the same maps, nothing more. An unknown,
revoked or expired token answers `401` on every request. Tokens themselves are managed
only from a browser session: `/api/me/tokens` refuses a bearer token, so a leaked one
cannot mint a replacement.

| Method   | Path                  | Body                                | Answer                          |
|----------|-----------------------|-------------------------------------|---------------------------------|
| `GET`    | `/api/me/tokens`      |                                     | `[{ id, name, last_used_at, expires_at, created_at }]` |
| `POST`   | `/api/me/tokens`      | `{ name, expires_in_days? }`        | `{ token, plaintext }`          |
| `DELETE` | `/api/me/tokens/{id}` |                                     | `null`                          |

## The whole API

With a token, every endpoint the frontend uses is open to a script: the routes are listed
in each `src/api/*.rs` module's `routes()` and the wire types are the `#[ts(export)]`
structs beside them (mirrored in `frontend/src/lib/api/types/`). That surface follows the
frontend and changes with it; a script that needs stability should prefer `/api/v1/`.

## `/api/v1/`

Responses are wrapped the way the legacy API wrapped them: reads in `{ "data": ... }`,
writes in `{ "message": "...", "data"?: ... }`, errors in `{ "error": "..." }` with the
matching status (`400` bad input, `401` no or bad token, `403` not allowed on this map,
`404` not there or not yours, `409` conflict).

### Sovereignty

`GET /api/v1/sovereignties`, no token needed. Every claimed system, keyed by system id:

```json
{
  "30000142": {
    "id": 30000142,
    "alliance": { "id": 99000001, "name": "Holders", "ticker": "HOLD" }
  }
}
```

Only the holder that applies is present (`alliance`, `corporation` or `faction`). Cached
for a day by the `Cache-Control` header.

### Maps

| Method        | Path                        | Body       |
|---------------|-----------------------------|------------|
| `GET`         | `/api/v1/maps?search=`      |            |
| `GET`         | `/api/v1/maps/{id}`         |            |
| `PUT`/`PATCH` | `/api/v1/maps/{id}`         | `{ name }` (manager or above) |

A map:

```json
{
  "id": 1,
  "name": "Chain",
  "home_solarsystem_id": 31000005,
  "rally_solarsystem_id": null,
  "layout": "manual",
  "allow_layout_override": false,
  "bookmark_format_wormhole": "{alias} {sig} {class}",
  "bookmark_format_kspace": "{alias} {class} {sig} {name} {region}",
  "bookmark_format_return": "*{alias} {sig} {class}",
  "bookmark_alias_scheme": "numeric",
  "bookmark_ignored_alias": "HOME",
  "map_solarsystems": [
    {
      "id": 10, "map_id": 1, "solarsystem_id": 31000005,
      "alias": "HOME", "status": "friendly", "occupier_alias": null,
      "position": { "x": 100, "y": 100 }, "pinned": true,
      "signatures_count": 4, "uncategorized_signatures_count": 1,
      "wormhole_signatures_count": 2, "map_connections_count": 1,
      "threat_level": null
    }
  ],
  "map_connections": [
    {
      "id": 5, "from_map_solarsystem_id": 10, "to_map_solarsystem_id": 11,
      "type": "wormhole", "preserve_mass": false,
      "mass_status": "fresh", "lifetime_status": "healthy",
      "lifetime_status_updated_at": null, "ship_size": "large",
      "jumps_mass_sum": 0, "jumps_count": 0,
      "signatures": [{ "id": 7, "signature_id": "ABC-123", "map_solarsystem_id": 10, "..." : "..." }],
      "created_at": "...", "updated_at": "..."
    }
  ],
  "owner": { "id": 90000001, "character_name": "Pilot" }
}
```

The vocabulary is the legacy one where it differed: `mass_status` is `fresh`, `reduced`
or `critical`; `lifetime_status` is `healthy`, `eol` or `critical`; `ship_size` is
`frigate`, `medium`, `large` or `xlarge`. `status` is `unknown`, `friendly`, `hostile`,
`active`, `unscanned` or `empty`. Ghost placements (a scanned hole nobody has jumped)
are not listed: the legacy API had no such thing.

### Map solar systems

| Method        | Path                              | Body |
|---------------|-----------------------------------|------|
| `GET`         | `/api/v1/map-solarsystems/{id}`   |      |
| `POST`        | `/api/v1/map-solarsystems`        | `{ map_id, solarsystem_id, position_x, position_y, alias?, occupier_alias?, status?, pinned? }` |
| `PUT`/`PATCH` | `/api/v1/map-solarsystems/{id}`   | `{ alias?, occupier_alias?, notes?, position_x?, position_y?, status?, pinned? }` |
| `DELETE`      | `/api/v1/map-solarsystems/{id}`   |      |

`{id}` is the placement id (`map_solarsystems[].id`), not the EVE system id. Writes need
member or above on the map, as they do on the screen, and each field becomes the same
undoable command a person would have issued. On update an absent field is left alone
and `null` clears it; `position_x` and `position_y` go together.

The read carries the placement with its signatures and the connections touching it.
`notes` is `null` for a viewer, who never sees them.

## Not carried over

The legacy `connect_to_map_solarsystem_id` on create, which drew a connection from an
existing placement while placing; use `POST /api/maps/{id}/connections/add` afterwards.
