-- New placements start as `unknown` (legacy default: no status icon, neutral border)
-- rather than `unscanned`, which is an explicit user choice.
alter table map_solar_system_details alter column status set default 'unknown';
