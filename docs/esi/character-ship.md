# Get current ship

[`GET /characters/{character_id}/ship`](https://developers.eveonline.com/api-explorer#/operations/GetCharactersCharacterIdShip)
· auth: scope `esi-location.read_ship_type.v1`

The character's current ship type, name, and instance id.

## Parameters

| Name | In | Required | Type | Description |
|------|----|:--------:|------|-------------|
| `character_id` | path | yes | integer | The ID of the character |

## Request example

```http
GET https://esi.evetech.net/characters/91234567/ship
Authorization: Bearer <token: esi-location.read_ship_type.v1>
X-Compatibility-Date: 2026-06-09
```

## Response — 200 OK

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `ship_item_id` | integer (int64) | yes | Unique to a ship instance; persists until repackaged. Use it to detect when a pilot changes into a different instance of the same ship type. |
| `ship_name` | string | yes | Player-given ship name |
| `ship_type_id` | integer (int64) | yes | SDE type id of the hull |

```json
{
  "ship_item_id": 1000000016991,
  "ship_name": "Rifter of Doom",
  "ship_type_id": 587
}
```

## In vector

Feeds the `ship_*` fields of
[`character_status`](../database/tracking.md#character_status). A changed
`ship_item_id` means the pilot swapped ships → bump `ship_updated_at`.
