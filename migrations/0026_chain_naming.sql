-- How a map names its chain.
--
-- These live on the map, not on the user: an alias is written on the map for everyone,
-- and a group whose members bookmark holes three different ways has a folder nobody can
-- read. The defaults are legacy's, so an existing map keeps naming things the way its
-- members already do by hand.
alter table maps
    add column alias_scheme text not null default 'numeric',
    -- The alias that is not part of the chain (the staging system). Children of it start
    -- a fresh sequence, and a bookmark pointing at it is a way home.
    add column ignored_alias text not null default 'HOME',
    add column bookmark_wormhole text not null default '{alias} {sig} {class}',
    add column bookmark_kspace   text not null default '{alias} {class} {sig} {name} {region}',
    -- The leading `*` sorts the way home to the top of the in-game folder.
    add column bookmark_return   text not null default '*{alias} {sig} {class}';

-- What the jump tracker does on this user's behalf. Whether tracking runs at all is
-- `tracking_allowed`, which already gates sharing the character's position.
alter table map_user_settings
    -- Off means a jump is mapped straight away with no signature; the hole still gets
    -- built, it just goes unlinked.
    add column prompt_for_signature boolean not null default true,
    add column suggest_alias boolean not null default true,
    -- Copying without being asked is the kind of thing that steals a clipboard mid-fight,
    -- so it is opt-in.
    add column copy_bookmark boolean not null default false;
