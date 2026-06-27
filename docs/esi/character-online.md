# Get character online

[`GET /characters/{character_id}/online`](https://developers.eveonline.com/api-explorer#/operations/GetCharactersCharacterIdOnline)
· auth: scope `esi-location.read_online.v1`

Checks whether the character is currently online.

## Parameters

| Name | In | Required | Type | Description |
|------|----|:--------:|------|-------------|
| `character_id` | path | yes | integer | The ID of the character |

## Request example

```http
GET https://esi.evetech.net/characters/91234567/online
Authorization: Bearer <token: esi-location.read_online.v1>
X-Compatibility-Date: 2026-06-09
```

## Response — 200 OK

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `online` | boolean | yes | Whether the character is online |
| `last_login` | string (date-time) | — | Timestamp of the last login |
| `last_logout` | string (date-time) | — | Timestamp of the last logout |
| `logins` | integer (int64) | — | Total number of times the character has logged in |

```json
{
  "online": true,
  "last_login": "2026-06-27T11:45:00Z",
  "last_logout": "2026-06-27T03:10:00Z",
  "logins": 4823
}
```

## In vector

Feeds [`character_status`](../database/tracking.md#character_status): `online`
directly, and `last_online_at` from `last_logout` (so the UI can show "last seen N
ago" while offline).
