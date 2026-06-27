# List sovereignty of systems

[`GET /sovereignty/systems`](https://developers.eveonline.com/api-explorer#/operations/GetSovereigntySystems)
· auth: **public**

Sovereignty details for all K-space solar systems in New Eden — who, if anyone, holds
each system.

## Parameters

None (returns the full list).

## Request example

```http
GET https://esi.evetech.net/sovereignty/systems
X-Compatibility-Date: 2026-06-09
```

## Response — 200 OK

An object with a `solar_systems` array; each entry has a `solar_system_id` and a
`claim` that is **one of** `faction`, `alliance`, or `unclaimed`.

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `solar_systems` | array | yes | One entry per K-space system |
| `solar_systems[].solar_system_id` | integer (int64) | yes | The system |
| `solar_systems[].claim` | one of below | yes | Faction / alliance / unclaimed |

**`claim` = alliance** (`SovereigntySystemsAlliance`): `alliance_id`, `corporation_id`,
`claimed_since` (date-time), `is_capital_system` (bool), `sovereignty_hub`,
`development`.

**`claim` = faction** (`SovereigntySystemsFaction`): `faction_id`.

**`claim` = unclaimed**: `unclaimed: true`.

```json
{
  "solar_systems": [
    {
      "solar_system_id": 30000142,
      "claim": {
        "alliance": {
          "alliance_id": 99000001,
          "corporation_id": 98000001,
          "claimed_since": "2026-01-04T18:00:00Z",
          "is_capital_system": true
        }
      }
    },
    { "solar_system_id": 30002813, "claim": { "faction": { "faction_id": 500001 } } },
    { "solar_system_id": 30000157, "claim": { "unclaimed": true } }
  ]
}
```

## In vector

Refreshes [`system_sovereignty`](../database/universe.md#system_sovereignty) — one row
per claimed system (alliance/corporation **or** faction). Pulled wholesale by
[sovereignty refresh](../processes.md#sovereignty-refresh) and displayed on the map to
show who owns a system.
