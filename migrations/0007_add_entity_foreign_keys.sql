-- Cross-entity foreign keys, added once every table exists. These reference the
-- ESI-cached entity tables (corporations, alliances, factions) and span migrations /
-- a corporations <-> alliances <-> factions cycle, so they can't all be inline.
--
-- The entity cross-references are DEFERRABLE INITIALLY DEFERRED: the cycle has real data
-- (a faction points to its NPC corp, that corp points back to its faction), so a seeder
-- inserts them in one transaction and the FKs are validated at commit.
--
-- `map_access.subject_id` stays unconstrained: it's polymorphic per `subject_type`.

alter table characters
    add foreign key (corporation_id) references corporations (id) deferrable initially deferred,
    add foreign key (alliance_id) references alliances (id) deferrable initially deferred;

alter table factions
    add foreign key (corporation_id) references corporations (id) deferrable initially deferred,
    add foreign key (militia_corporation_id) references corporations (id) deferrable initially deferred;

alter table corporations
    add foreign key (alliance_id) references alliances (id) deferrable initially deferred,
    add foreign key (faction_id) references factions (id) deferrable initially deferred;

alter table alliances
    add foreign key (creator_corporation_id) references corporations (id) deferrable initially deferred,
    add foreign key (executor_corporation_id) references corporations (id) deferrable initially deferred,
    add foreign key (faction_id) references factions (id) deferrable initially deferred;

alter table stations
    add foreign key (owner_corporation_id) references corporations (id) deferrable initially deferred;

alter table structures
    add foreign key (owner_corporation_id) references corporations (id) deferrable initially deferred;

alter table system_sovereignty
    add foreign key (alliance_id) references alliances (id) deferrable initially deferred,
    add foreign key (corporation_id) references corporations (id) deferrable initially deferred;

alter table character_status
    add foreign key (station_id) references stations (id) deferrable initially deferred,
    add foreign key (structure_id) references structures (id) deferrable initially deferred;
