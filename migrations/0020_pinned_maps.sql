-- Quick access: the maps a person keeps in the top bar.
--
-- Per user rather than per map: which chains you are flying this week is your business,
-- and two people on the same map rarely want the same shortcuts.
alter table map_user_settings add column is_pinned boolean not null default false;
