-- Whether the map keeps the side cards on the system this user's pilot is in, selecting
-- each arrival as the jump tracker records it. Off by default: it moves the selection out
-- from under whoever is reading a different system.
alter table map_user_settings add column follow_character boolean not null default false;
