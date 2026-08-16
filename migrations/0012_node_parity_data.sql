-- Data foundation for legacy node parity: wormhole scan strength, the shattered flag,
-- the legacy status vocabulary, and a seed revision so data-format changes re-seed
-- without a new SDE build.

alter table wormhole_types add column signature_strength double precision;

alter table wormhole_systems add column is_shattered boolean not null default false;

-- Legacy status set: unknown / friendly / hostile / active / unscanned / empty.
update map_solar_system_details set status = 'unscanned' where status = 'scanned';
update map_solar_system_details set status = 'active' where status = 'occupied';

alter table sde_build add column seed_revision int not null default 0;
