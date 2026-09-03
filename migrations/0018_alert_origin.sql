-- Proximity only: an optional fixed starting point. Without one the alert measures from
-- whichever mapped system is nearest the target; with one it measures the route from
-- here to the target through the chain, so "home within five jumps of X" stays about home
-- rather than about whatever exit happened to land nearby.
alter table map_alerts
    add column origin_solar_system_id bigint references solar_systems (id) on delete set null;
