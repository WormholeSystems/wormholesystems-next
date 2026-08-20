# Character affiliation (bulk)

[`POST /characters/affiliation`](https://developers.eveonline.com/api-explorer#/operations/PostCharactersAffiliation)
· auth: **public**

Bulk lookup of character ids to their corporation, alliance, and faction.

## Parameters

None in the path/query. The character ids go in the request body.

## Request body

A JSON array of unique character ids — **1 to 1000** per call (`minItems` 1,
`maxItems` 1000, `uniqueItems`):

```json
[91234567, 91234568, 91234569]
```

```http
POST https://esi.evetech.net/characters/affiliation
Content-Type: application/json
X-Compatibility-Date: 2026-06-09
```

## Response — 200 OK

An array, one object per character:

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `character_id` | integer (int64) | yes | The character's ID |
| `corporation_id` | integer (int64) | yes | The character's corporation ID |
| `alliance_id` | integer (int64) | — | Alliance ID, if the corporation is in an alliance |
| `faction_id` | integer (int64) | — | Faction ID, if the corporation is in a faction |

```json
[
  { "character_id": 91234567, "corporation_id": 98000001, "alliance_id": 99000001 },
  { "character_id": 91234568, "corporation_id": 98000042 }
]
```

## In WormholeSystems

The efficient way to keep [`characters`](../database/authentication.md#characters)
`.corporation_id` / `.alliance_id` current — which directly drives
[access checks](../database/access.md#map_access). Refresh many characters in one call
instead of hitting the per-character endpoint.
