# Set autopilot waypoint

[`POST /ui/autopilot/waypoint`](https://developers.eveonline.com/api-explorer#/operations/PostUiAutopilotWaypoint)
· auth: scope `esi-ui.write_waypoint.v1`

Sets a waypoint in the autopilot of the **logged-in EVE client** for the character
whose token is used. That client must be running; this writes to the game UI, it does
not return data.

## Parameters

| Name | In | Required | Type | Description |
|------|----|:--------:|------|-------------|
| `destination_id` | query | yes | integer | Destination to travel to — a solar system, station, or structure id |
| `add_to_beginning` | query | yes | boolean | Prepend to the existing route (`true`) vs. append (`false`) |
| `clear_other_waypoints` | query | yes | boolean | Clear the existing route before adding |

## Request example

```http
POST https://esi.evetech.net/ui/autopilot/waypoint?destination_id=30000142&add_to_beginning=false&clear_other_waypoints=true
Authorization: Bearer <token: esi-ui.write_waypoint.v1>
X-Compatibility-Date: 2026-06-09
```

## Response — 204 No Content

No response body.

## In WormholeSystems

Backs the "set destination / route home" action from the map — e.g. setting the
shortest path out of the chain on the active character's autopilot.
