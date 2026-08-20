# Get alliance public information

[`GET /alliances/{alliance_id}`](https://developers.eveonline.com/api-explorer#/operations/GetAlliancesAllianceId)
· auth: **public**

Public information about an alliance.

## Parameters

| Name | In | Required | Type | Description |
|------|----|:--------:|------|-------------|
| `alliance_id` | path | yes | integer | The ID of the alliance |

## Request example

```http
GET https://esi.evetech.net/alliances/99000001
X-Compatibility-Date: 2026-06-09
```

## Response — 200 OK

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `name` | string | yes | Alliance's name |
| `ticker` | string | yes | Alliance's ticker |
| `creator_corporation_id` | integer (int64) | yes | Corporation that created the alliance |
| `creator_id` | integer (int64) | yes | Character that created the alliance |
| `date_founded` | string (date-time) | yes | Founding date |
| `executor_corporation_id` | integer (int64) | — | Executor corporation, if the alliance still has one |
| `faction_id` | integer (int64) | — | Faction, if any |

```json
{
  "name": "WormholeSystems Coalition",
  "ticker": "VCTR",
  "creator_corporation_id": 98000001,
  "creator_id": 91234567,
  "date_founded": "2019-01-15T20:00:00Z",
  "executor_corporation_id": 98000001
}
```

## In WormholeSystems

Resolves an alliance's name/ticker for display of
[`map_access`](../database/access.md#map_access) entries.
