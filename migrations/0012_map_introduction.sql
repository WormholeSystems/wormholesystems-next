-- When this user finished the map's introduction. Separate from `setup_dismissed_at`: the
-- introduction is the one-time walkthrough of permissions and preferences, the setup guide
-- is the standing checklist of things the map still needs.
alter table map_user_settings add column introduction_confirmed_at timestamptz;
