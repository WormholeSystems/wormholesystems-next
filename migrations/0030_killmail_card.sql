-- The killmails card.
--
-- The query it drives is "what died recently, in any of these systems", which is a range
-- scan per system. The existing indexes cover one half each: `killmails_system_time` is
-- (system, time) ascending and `killmails_time` is time alone. This is the composite the
-- card actually wants, newest first.
create index killmails_system_recent on killmails (solar_system_id, time desc);

-- Which half of the chain to show kills from. `all` / `jspace` / `kspace`, matching legacy.
alter table map_user_settings
    add column killmail_filter text not null default 'all';
