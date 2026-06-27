-- Cross-entity foreign keys, added once every table exists. These reference the
-- ESI-cached entity tables (corporations, alliances, factions) and span migrations /
-- a corporations <-> alliances <-> factions cycle, so they can't all be inline.
--
-- Enforcing these means loaders must populate referenced rows first — e.g. seed NPC
-- corporations before stations, upsert a character's corp before the character.
-- `map_access.subject_id` stays unconstrained: it's polymorphic per `subject_type`.

alter table characters
    add foreign key (corporation_id) references corporations (id),
    add foreign key (alliance_id) references alliances (id);

alter table factions
    add foreign key (corporation_id) references corporations (id),
    add foreign key (militia_corporation_id) references corporations (id);

alter table corporations
    add foreign key (alliance_id) references alliances (id),
    add foreign key (faction_id) references factions (id);

alter table alliances
    add foreign key (creator_corporation_id) references corporations (id),
    add foreign key (executor_corporation_id) references corporations (id),
    add foreign key (faction_id) references factions (id);

alter table stations
    add foreign key (owner_corporation_id) references corporations (id);

alter table structures
    add foreign key (owner_corporation_id) references corporations (id);

alter table system_sovereignty
    add foreign key (alliance_id) references alliances (id),
    add foreign key (corporation_id) references corporations (id);

alter table character_status
    add foreign key (station_id) references stations (id),
    add foreign key (structure_id) references structures (id);
