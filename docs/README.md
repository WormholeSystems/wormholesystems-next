# vector — documentation

`vector` is a Rust rewrite of [**WormholeSystems**](https://wormhole.systems/), a
real-time, collaborative wormhole-mapping tool for EVE Online.

It lets a corp or alliance map a wormhole chain together: pilots share one live map,
paste probe-scanner results to sync signatures, track the mass and lifetime state of
each connection, see where each other are via ESI, and find the shortest way home —
with layered access control and Discord alerting.

## Contents

- [`database/`](./database/) — the application data model: goals, tables,
  relationships, and expected behaviour, split by domain
  ([mapping](./database/mapping.md), [authentication](./database/authentication.md),
  [access](./database/access.md), [tracking](./database/tracking.md),
  [universe](./database/universe.md), [item types](./database/types.md),
  [custom static reference](./database/static.md)).
- [`features/`](./features/) — application behaviour above the data model: the
  authorized, validated actions users take. ([map actions](./features/maps.md) —
  maps, access, graph editing, connections, and signatures; [realtime](./features/realtime.md)
  — the in-process event bus that pushes map changes to viewers.)
- [`esi/`](./esi/) — the EVE ESI endpoints we consume: parameters, response
  structure, and examples, one file per endpoint.
- [`processes.md`](./processes.md) — background / scheduled work (e.g. affiliation
  refresh) and the data it touches.
