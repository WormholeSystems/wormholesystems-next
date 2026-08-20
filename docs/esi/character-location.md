# Get character location

[`GET /characters/{character_id}/location`](https://developers.eveonline.com/api-explorer#/operations/GetCharactersCharacterIdLocation)
· auth: scope `esi-location.read_location.v1`

Information about the character's current location: the current solar system id, plus
the current station or structure id when docked.

## Parameters

| Name | In | Required | Type | Description |
|------|----|:--------:|------|-------------|
| `character_id` | path | yes | integer | The ID of the character |

## Request example

```http
GET https://esi.evetech.net/characters/91234567/location
Authorization: Bearer <token: esi-location.read_location.v1>
X-Compatibility-Date: 2026-06-09
```

## Response — 200 OK

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `solar_system_id` | integer (int64) | yes | Current solar system |
| `station_id` | integer (int64) | — | Present only when docked in a station |
| `structure_id` | integer (int64) | — | Present only when docked in a structure |

```json
{
  "solar_system_id": 30000142,
  "station_id": 60003760
}
```

In space, neither `station_id` nor `structure_id` is present.

## In WormholeSystems

Feeds [`character_status`](../database/tracking.md#character_status): `solar_system_id`
maps directly; `is_docked` is **derived** as `station_id IS NOT NULL OR structure_id IS
NOT NULL`.
