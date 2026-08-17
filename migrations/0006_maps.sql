-- ---------------------------------------------------------------------------
-- Maps: the graph, its intel, and who may see it
-- ---------------------------------------------------------------------------

-- `head_event_id` is the map's cursor into its own history tree; the foreign key is added
-- after `map_events` exists, since the two reference each other.
create table maps (
    id                bigint generated always as identity primary key,
    name              text not null,
    description       text,
    image_url         text,
    -- How this map names its chain. Map-wide rather than per-user, because an alias is
    -- written on the map for everyone and a bookmark folder in three conventions is
    -- unreadable. The defaults match what most groups already do by hand.
    alias_scheme      text not null default 'numeric',
    -- The alias that sits outside the chain (the staging system). Its holes start a fresh
    -- sequence, and a bookmark pointing back at it is a way home.
    ignored_alias     text not null default 'HOME',
    bookmark_wormhole text not null default '{alias} {sig} {class}',
    bookmark_kspace   text not null default '{alias} {class} {sig} {name} {region}',
    -- The leading `*` sorts the way home to the top of the in-game folder.
    bookmark_return   text not null default '*{alias} {sig} {class}',
    head_event_id     bigint,
    created_at        timestamptz not null default now()
);

-- Ephemeral placement: a system as currently on the map.
create table map_solar_systems (
    id              bigint generated always as identity primary key,
    map_id          bigint not null references maps (id) on delete cascade,
    solar_system_id bigint not null references solar_systems (id),
    position_x      double precision not null,
    position_y      double precision not null,
    alias           text,
    -- A map has at most one home system and at most one rally point (partial unique
    -- indexes below) but any number of pinned systems. Pinned systems are drag-locked and
    -- survive "clear map".
    is_home         boolean not null default false,
    is_rally        boolean not null default false,
    is_pinned       boolean not null default false,
    created_at      timestamptz not null default now(),

    unique (map_id, solar_system_id)
);

create unique index map_solar_systems_one_home
    on map_solar_systems (map_id) where is_home;
create unique index map_solar_systems_one_rally
    on map_solar_systems (map_id) where is_rally;

-- Persisted intel: survives a system being removed from the map. A new placement starts
-- as `unknown` (no status icon, neutral border); anything else is an explicit choice.
create table map_solar_system_details (
    id              bigint generated always as identity primary key,
    map_id          bigint not null references maps (id) on delete cascade,
    solar_system_id bigint not null references solar_systems (id),
    status          text not null default 'unknown',
    occupying_group text,
    notes           text,
    updated_at      timestamptz not null default now(),

    unique (map_id, solar_system_id)
);

-- A wormhole's life-cycle state lives here *and* on its signatures; the triggers further
-- down keep a linked group in agreement. `preserve_mass` excludes the hole from mass
-- bookkeeping.
create table map_connections (
    id                     bigint generated always as identity primary key,
    map_id                 bigint not null references maps (id) on delete cascade,
    from_system            bigint not null references map_solar_systems (id) on delete cascade,
    to_system              bigint not null references map_solar_systems (id) on delete cascade,
    type                   text not null,
    mass_status            text,
    time_status            text,
    size                   text,
    -- When the lifetime last changed, for "EOL since" ages. Stamped by trigger so every
    -- write path is covered, including the sync below.
    time_status_updated_at timestamptz,
    preserve_mass          boolean not null default false,
    created_at             timestamptz not null default now(),
    updated_at             timestamptz not null default now(),

    check (from_system <> to_system)
);

create table signatures (
    id                     bigint generated always as identity primary key,
    map_id                 bigint not null,
    solar_system_id        bigint not null,
    signature_id           text not null,
    "group"                text not null,
    -- The matched catalog type; `name` keeps the raw scanner text when nothing matched.
    signature_type_id      bigint references signature_types (id),
    name                   text,
    size                   text,
    mass_status            text,
    time_status            text,
    time_status_updated_at timestamptz,
    connection_id          bigint references map_connections (id) on delete set null,
    created_at             timestamptz not null default now(),
    updated_at             timestamptz not null default now(),

    unique (map_id, solar_system_id, signature_id),
    -- Tied to the placement, so removing a system deletes its signatures.
    foreign key (map_id, solar_system_id)
        references map_solar_systems (map_id, solar_system_id) on delete cascade
);

-- `subject_id` is a character / corporation / alliance EVE id (polymorphic per
-- `subject_type`), so it cannot reference a single table.
create table map_access (
    id           bigint generated always as identity primary key,
    map_id       bigint not null references maps (id) on delete cascade,
    subject_type text not null,
    subject_id   bigint not null,
    role         text not null,
    created_at   timestamptz not null default now(),

    unique (map_id, subject_id)
);

-- Per-user, per-map preferences: what to share, what to show, and how the page is laid
-- out. `tracking_allowed` is the explicit opt-in for sharing this user's characters' live
-- location on this map, and gates everything else that watches them.
create table map_user_settings (
    map_id                  bigint not null references maps (id) on delete cascade,
    user_id                 bigint not null references users (id) on delete cascade,
    tracking_allowed        boolean not null default false,
    show_threat_level       boolean not null default true,
    compact_signature_list  boolean not null default false,
    show_statics_first      boolean not null default false,
    -- Route calculation. The tolerance columns store the worst status still routed
    -- through: time 'critical' = anything, mass 'reduced' = fresh or reduced holes only.
    route_preference        text not null default 'shorter',
    security_penalty        int not null default 50,
    route_allow_time_status text not null default 'critical',
    route_allow_mass_status text not null default 'reduced',
    route_use_evescout      boolean not null default false,
    -- What the jump tracker does on this user's behalf. Off, a jump is mapped straight
    -- away with no signature: the hole still gets built, it just goes unlinked.
    prompt_for_signature    boolean not null default true,
    suggest_alias           boolean not null default true,
    -- Copying without being asked is the kind of thing that steals a clipboard mid-fight.
    copy_bookmark           boolean not null default false,
    -- Which half of the chain the killmails card shows: all / jspace / kspace.
    killmail_filter         text not null default 'all',
    -- Hides a finished chain from this user's map list without deleting it for everyone
    -- else. Per-user because one group's dead chain is another's staging map.
    is_archived             boolean not null default false,
    -- Panels this user hides, and the per-breakpoint tile positions. Null layout means
    -- "the built-in arrangement", so a map nobody has customised renders from defaults.
    hidden_panels           text[] not null default '{}',
    layout_breakpoints      jsonb,
    updated_at              timestamptz not null default now(),

    primary key (map_id, user_id)
);

-- Every observed or manually logged transit, with the hull's mass, feeding a connection's
-- mass-remaining estimate. A null `connection_id` is a pending row observed before the
-- hole was mapped; it is claimed by a matching connection created shortly after, or pruned.
create table map_connection_jumps (
    id                   bigint generated always as identity primary key,
    map_id               bigint not null references maps (id) on delete cascade,
    connection_id        bigint references map_connections (id) on delete cascade,
    -- Kept when the character disappears, so the mass ledger survives.
    character_id         bigint references characters (id) on delete set null,
    from_solar_system_id bigint not null references solar_systems (id),
    to_solar_system_id   bigint not null references solar_systems (id),
    ship_type_id         bigint references types (id) on delete set null,
    ship_name            text,
    mass                 bigint not null default 0,
    is_manual            boolean not null default false,
    created_at           timestamptz not null default now(),
    updated_at           timestamptz not null default now()
);

create index map_connection_jumps_claim
    on map_connection_jumps (map_id, from_solar_system_id, to_solar_system_id, created_at);
create index map_connection_jumps_by_connection
    on map_connection_jumps (connection_id, created_at);

-- Systems whose distance is tracked on a map.
create table map_watchlist (
    id              bigint generated always as identity primary key,
    map_id          bigint not null references maps (id) on delete cascade,
    solar_system_id bigint not null references solar_systems (id),
    is_pinned       boolean not null default false,
    created_at      timestamptz not null default now(),

    unique (map_id, solar_system_id)
);

-- The command journal, as a history tree with a cursor.
--
-- One row per applied mutation, recorded in the same transaction as the write it
-- describes. `inverse` and `forward` are serialized MapCommands, so undo is just another
-- execution and redo is undoing the undo; both are refreshed as the cursor crosses a step,
-- which is what lets a step be walked back and forth without new rows accumulating.
create table map_events (
    id            bigint generated always as identity primary key,
    map_id        bigint not null references maps (id) on delete cascade,
    -- Null = applied by a background task (expiry, jump capture).
    character_id  bigint references characters (id) on delete set null,
    kind          text not null,
    label         text not null,
    entries_count int not null default 1,
    inverse       jsonb,
    forward       jsonb,
    -- The step that was current when this one was applied; null = a root. `set null`
    -- rather than cascade, so retention can drop old ancestors without taking their
    -- descendants with them: the oldest survivor becomes a new root and undo stops there.
    parent_id     bigint references map_events (id) on delete set null,
    -- Whether this row is a step in the tree at all. Background writers record for the
    -- audit trail without becoming undoable steps.
    is_step       boolean not null default false,
    created_at    timestamptz not null default now()
);

create index map_events_by_map on map_events (map_id, created_at desc);
create index map_events_children on map_events (map_id, parent_id) where is_step;

-- The cursor. Null means every step has been undone.
alter table maps
    add foreign key (head_event_id) references map_events (id) on delete set null;
