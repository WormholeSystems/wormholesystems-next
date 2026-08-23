-- Who holds a system, answered once.
--
-- Three read paths (the map read, system search, system resolve) each carried the same
-- case/coalesce over four joins to turn a `system_sovereignty` row into a kind, id, name
-- and ticker, alliance preferred over corporation over faction. A view rather than a
-- repeated block, so a fourth reader cannot get the precedence subtly wrong.
create view system_sovereignty_resolved as
    select sov.solar_system_id,
           case
               when sov.alliance_id is not null then 'alliance'
               when sov.corporation_id is not null then 'corporation'
               when sov.faction_id is not null then 'faction'
           end as kind,
           coalesce(sov.alliance_id, sov.corporation_id, sov.faction_id) as entity_id,
           coalesce(al.name, co.name, f.name) as name,
           coalesce(al.ticker, co.ticker) as ticker
    from system_sovereignty sov
    left join alliances al on al.id = sov.alliance_id
    left join corporations co on co.id = sov.corporation_id
    left join factions f on f.id = sov.faction_id;
