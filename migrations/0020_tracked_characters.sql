-- Which of this user's pilots build this map as they fly. Empty means the pilot the
-- session is acting as, which is what every map did before; a chosen set lets a farm alt
-- map the farm chain without its jumps landing on the main map, and several pilots map
-- at once.
alter table map_user_settings
    add column tracked_character_ids bigint[] not null default '{}';
