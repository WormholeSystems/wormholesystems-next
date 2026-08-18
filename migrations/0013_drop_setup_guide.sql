-- The setup checklist is gone: the map introduction covers the same ground properly, and
-- two things asking to be set up at once was one too many.
alter table map_user_settings drop column setup_dismissed_at;
