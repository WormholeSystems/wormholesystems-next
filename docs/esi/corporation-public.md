# Get corporation public information

[`GET /corporations/{corporation_id}`](https://developers.eveonline.com/api-explorer#/operations/GetCorporationsCorporationId)
· auth: **public**

Public information about a corporation.

## Parameters

| Name | In | Required | Type | Description |
|------|----|:--------:|------|-------------|
| `corporation_id` | path | yes | integer | The ID of the corporation |

## Request example

```http
GET https://esi.evetech.net/corporations/98000001
X-Compatibility-Date: 2026-06-09
```

## Response — 200 OK

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `name` | string | yes | Corporation's name |
| `ticker` | string | yes | Corporation's short name (ticker) |
| `alliance_id` | integer (int64) | — | Alliance, if any |
| `member_count` | integer (int64) | yes | Member count |
| `ceo_id` | integer (int64) | yes | CEO character id |
| `creator_id` | integer (int64) | yes | Founder character id |
| `tax_rate` | number (double) | yes | Tax rate |
| `date_founded` | string (date-time) | — | Founding date |
| `home_station_id` | integer (int64) | — | Home station |
| `faction_id` | integer (int64) | — | Faction, if any |
| `shares` | integer (int64) | — | Number of shares |
| `url` | string | — | Corporation URL |
| `war_eligible` | boolean | — | Whether war-eligible |
| `description` | string | — | Corporation description |

```json
{
  "name": "WormholeSystems Holdings",
  "ticker": "VCTR",
  "alliance_id": 99000001,
  "member_count": 142,
  "ceo_id": 91234567,
  "creator_id": 91234567,
  "tax_rate": 0.1,
  "date_founded": "2018-09-01T18:00:00Z"
}
```

## In WormholeSystems

Resolves a corporation's name/ticker for display of
[`map_access`](../database/access.md#map_access) entries (we store ids, render names).
