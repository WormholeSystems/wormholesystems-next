-- Per-system notes (markdown), part of the persisted intel that survives removal and
-- re-adding of a system. Hidden from viewers; served through a member-gated endpoint.
alter table map_solar_system_details add column notes text;
