-- Per-user panel layout for a map's sidebar: which cards are hidden, and the order the
-- rest appear in. Both default to empty, which means "the built-in layout" — so an
-- untouched map keeps rendering exactly as it did before anyone edited anything.
alter table map_user_settings
    add column hidden_panels text[] not null default '{}',
    add column panel_order   text[] not null default '{}';
