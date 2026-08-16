-- Per-user, per-map preferences. `tracking_allowed` is the explicit opt-in for sharing
-- the user's characters' live location on this map (legacy default: off).
create table map_user_settings (
    map_id     bigint not null,
    user_id    bigint not null,
    tracking_allowed  boolean not null default false,
    show_threat_level boolean not null default true,
    updated_at timestamptz not null default now(),

    primary key (map_id, user_id),
    foreign key (map_id) references maps (id) on delete cascade,
    foreign key (user_id) references users (id) on delete cascade
);
