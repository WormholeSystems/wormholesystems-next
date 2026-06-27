# Background Processes

Scheduled / background work the server runs outside of request handling. One section
per job: its purpose, triggers, and the data it touches.

## Affiliation refresh

Keeps each [`characters`](./database/authentication.md#characters) row's
`corporation_id` / `alliance_id` current. This matters because those fields **drive
[access](./database/access.md)**: a map can grant access to a whole corporation or
alliance, so when a character's affiliation changes, what maps they can reach must
change too. Both triggers use the bulk
[character affiliation](./esi/characters-affiliation.md) endpoint.

### On authentication

When a user logs in — or links / re-authenticates a character — we refetch the
affiliation of the user's characters and update their corp/alliance **before**
resolving access. So someone who just left a corporation can't get in on stale data at
login.

### Periodic (the background job)

Users who don't log in would otherwise keep stale affiliations indefinitely, so a
scheduled job refetches the affiliations of **all** characters assigned to users:

- Batch every character id through `POST /characters/affiliation` (≤ 1000 ids per
  call), update `corporation_id` / `alliance_id`, and bump `updated_at`.
- A character that has left its corporation/alliance stops matching that corp/alliance
  `map_access` grant on the next run → **access is lost within one refresh interval.**

**Invariants & expected behaviour**

- After a run, every character's stored corp/alliance equals what ESI reports.
- A character no longer in a granted corporation/alliance loses that
  affiliation-derived access at the next run — bounded staleness equals the interval.
- **Direct character-id grants are unaffected** by affiliation changes; they persist
  until explicitly removed.

> **Open — interval.** How fresh must access be — i.e. the maximum acceptable lag
> between someone leaving a corp and losing access? That sets the job interval (e.g.
> 15 min vs. hourly), and whether each run is a full sweep or staggered batches.

## Character status polling

Keeps [`character_status`](./database/tracking.md#character_status) live for characters
whose user is actively using the app — without burning ESI calls on idle users or
offline characters. A **two-tier** poll, gated by app activity.

**Eligibility — active users only.** We poll only the characters of a user who has
interacted with the app **within the last 5 minutes**, measured by
[`users.last_active_at`](./database/authentication.md#users). When a user goes idle
past that window, *all* polling for their characters stops.

**Tier 1 — online state, every 60 s.** For each eligible character, call
[`GET …/online`](./esi/character-online.md) (`esi-location.read_online.v1`) and update
`online` / `last_online_at`.

**Tier 2 — location + ship, every 5 s.** *Only for characters currently online*, call
[`GET …/location`](./esi/character-location.md) and
[`GET …/ship`](./esi/character-ship.md), updating `solar_system_id` +
station/structure (→ derived `is_docked`) and the `ship_*` fields.

**Transitions**

- A character seen **offline** in Tier 1 drops out of Tier 2 until it is online again.
- A character that comes **online** in Tier 1 enters Tier 2 on the next 5 s tick.
- A user that goes **idle** (>5 min) removes all their characters from both tiers.

**Invariants & expected behaviour**

- A character is in Tier 2 **iff** it is online *and* its user is active (<5 min).
- An offline character is never polled for location/ship.
- An idle user's characters are not polled at all (neither tier).
- Each poll needs the matching scope (online / location / ship); a missing scope skips
  that field, which stays `null` (see [scopes](./database/authentication.md#scopes)).
- Cadences track ESI's cache windows — honour each response's cache-expiry / `ETag`
  rather than re-requesting inside a cached window.

## Sovereignty refresh

Keeps [`system_sovereignty`](./database/universe.md#system_sovereignty) current — who
holds each K-space system — for display on the map.

- Periodically pull [`GET /sovereignty/systems`](./esi/sovereignty-systems.md) (public,
  whole-of-New-Eden in one call) and upsert one row per claimed system: alliance +
  corporation, or faction; clear systems that became unclaimed.
- Sovereignty changes slowly relative to presence, so this runs on a coarse cadence
  (not the per-second tier of status polling).

**Invariants & expected behaviour**

- After a run, every system's stored holder equals what ESI reports; flipped systems
  reflect their new owner; unclaimed systems carry no holder.

> **Open — interval.** Refresh cadence for sovereignty (e.g. every few minutes vs.
> hourly) — slower than presence, but how slow?

## See also — other periodic work

Documented with their own tables; listed here so the background-work picture is in one
place:

- Access-token refresh from the stored refresh token
  ([`tokens`](./database/authentication.md#tokens)).
- Expiry / cleanup of expired
  [`oauth_login_flows`](./database/authentication.md#oauth_login_flows).
