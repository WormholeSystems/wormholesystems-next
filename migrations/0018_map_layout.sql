-- Automatic placement: the chain drawn as a tree instead of dragged into shape.
--
-- The mode is the map's, so everyone looking at the same chain sees the same shape. A map
-- may hand the choice to each viewer instead (`allow_layout_override`), which is what the
-- per-user column below holds. Positions are derived on the client and never stored: the
-- manual positions stay exactly as they were left.
alter table maps add column layout text not null default 'manual';
alter table maps add column allow_layout_override boolean not null default false;

alter table map_user_settings add column layout_override text;
