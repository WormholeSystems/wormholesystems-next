-- Whether the chain's own holes count as route edges. On by default: routing through the
-- chain is the point of the map. Turned off, a route is stargates only, so it stops
-- doubling back through a hole that a pilot on the other side would have to re-scan.
-- EVE Scout keeps its own switch, so Thera can still be routed through with this off.
alter table map_user_settings add column route_use_wormholes boolean not null default true;
