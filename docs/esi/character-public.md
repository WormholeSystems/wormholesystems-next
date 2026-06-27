# Get character public information

[`GET /characters/{character_id}`](https://developers.eveonline.com/api-explorer#/operations/GetCharactersDetail)
· auth: **public**

Public information about a character.

## Parameters

| Name | In | Required | Type | Description |
|------|----|:--------:|------|-------------|
| `character_id` | path | yes | integer | The ID of the character |

## Request example

```http
GET https://esi.evetech.net/characters/91234567
X-Compatibility-Date: 2026-06-09
```

## Response — 200 OK

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `name` | string | yes | Character's name |
| `corporation_id` | integer (int64) | yes | Current corporation |
| `alliance_id` | integer (int64) | — | Current alliance, if any |
| `faction_id` | integer (int64) | — | Faction, if any |
| `birthday` | string (date-time) | yes | Character's creation date |
| `gender` | string | yes | `male` or `female` |
| `race_id` | integer (int64) | yes | SDE race id |
| `bloodline_id` | integer (int64) | yes | SDE bloodline id |
| `achievement_score` | integer (int64) | yes | Character's achievement score |
| `security_status` | number (double) | — | Character's security status |
| `corporation_title` | string | — | Title within the corporation |
| `character_title_id` | string (uuid) | — | Title id |
| `description` | string | — | Biography |

```json
{
  "name": "Vector Pilot",
  "corporation_id": 98000001,
  "alliance_id": 99000001,
  "birthday": "2015-03-24T11:00:00Z",
  "gender": "male",
  "race_id": 2,
  "bloodline_id": 7,
  "achievement_score": 1840,
  "security_status": 4.7
}
```

## In vector

Resolves a character's name and current corp/alliance. For keeping the corp/alliance of
*many* characters fresh, prefer the bulk
[affiliation endpoint](./characters-affiliation.md).
