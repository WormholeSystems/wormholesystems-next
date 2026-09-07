-- A picture behind a viewer's own map. Per user, per map: it is a way of seeing the
-- chain, not part of the chain. The path is relative to the uploads directory so the
-- directory can move without touching a row.
create type map_background_mode as enum ('grid', 'viewport');

alter table map_user_settings
    add column background_image_path text,
    add column background_image_mode map_background_mode not null default 'grid';
