-- ---------------------------------------------------------------------------
-- Maps: the graph, its intel, and who may see it
-- ---------------------------------------------------------------------------

-- The vocabularies the map is written in. Types rather than `text` so the database is the
-- one saying what a column may hold: a status nobody defined cannot be written at all, and
-- the ones whose order means something compare correctly without a lookup table, because
-- Postgres orders an enum by the order its labels are declared in.
--
-- Adding a label later is `alter type ... add value`, which cannot run inside a
-- transaction. Migrations do, so a new variant needs its own migration marked
-- `-- no-transaction`.
create type map_role as enum ('viewer', 'member', 'manager', 'owner');
create type subject_type as enum ('character', 'corporation', 'alliance');
create type connection_type as enum ('wormhole', 'stargate');
create type alias_scheme as enum ('numeric', 'alphabetical');
create type map_layout as enum ('manual', 'tree');
create type route_preference as enum ('shorter', 'safer', 'less_secure');
create type killmail_scope as enum ('all', 'jspace', 'kspace');
create type system_status as enum ('unknown', 'friendly', 'hostile', 'active', 'unscanned', 'empty');
create type signature_group as enum
    ('wormhole', 'data', 'relic', 'gas', 'combat', 'ore', 'homefront', 'unknown');

-- Worst last on all three: the sync trigger picks the worst state in a connection's group
-- with a plain `order by ... desc`, which is only correct because of this order.
create type mass_status as enum ('stable', 'reduced', 'critical');
create type time_status as enum ('stable', 'eol', 'critical');
-- Biggest first, so "worst" is the most restrictive.
create type wormhole_size as enum ('xl', 'large', 'medium', 'small');

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
    alias_scheme      alias_scheme not null default 'numeric',
    -- The alias that sits outside the chain (the staging system). Its holes start a fresh
    -- sequence, and a bookmark pointing back at it is a way home.
    ignored_alias     text not null default 'HOME',
    bookmark_wormhole text not null default '{alias} {sig} {class}',
    bookmark_kspace   text not null default '{alias} {class} {sig} {name} {region}',
    -- The leading `*` sorts the way home to the top of the in-game folder.
    bookmark_return   text not null default '*{alias} {sig} {class}',
    head_event_id     bigint,
    -- Whether a scanned wormhole that leads nowhere yet is drawn as a node. Map-wide: a
    -- ghost is something everyone on the chain sees, so it cannot be one person's taste.
    ghost_unlinked_wormholes boolean not null default false,
    -- Automatic placement, so everyone looking at the same chain sees the same shape. A
    -- map may hand the choice to each viewer instead. Positions are derived on the client
    -- and never stored: the manual ones stay exactly as they were left.
    layout            map_layout not null default 'manual',
    allow_layout_override boolean not null default false,
    -- Two read-only ways in without an account. `is_public` puts the map in the open for
    -- anyone with the link; `share_token` keeps it private but lets whoever holds the
    -- secret watch. Either way the visitor is a viewer and pilots stay hidden.
    is_public         boolean not null default false,
    share_token       text unique,
    created_at        timestamptz not null default now()
);

-- Ephemeral placement: a system as currently on the map.
-- A placement with no solar system is a **ghost**: the far side of a scanned wormhole,
-- before anyone has been through it (docs/database/mapping.md#ghost-placements).
-- Connections, positions and aliases work on it unchanged, and `unique (map_id,
-- solar_system_id)` still caps real systems at one per map, because nulls are distinct.
--
-- The two `raised_by`/`hangs_off` columns are what a ghost is: the scan it was drawn for
-- and the placement that scan was made in. Both cascade, so the rules that govern a
-- ghost's life belong to the database rather than to whichever write remembers them.
-- Deferred, because restoring a removal walks a cycle: a ghost names its signature, a
-- signature names its connection, and a connection names its endpoints.
create table map_solar_systems (
    id              bigint generated always as identity primary key,
    map_id          bigint not null references maps (id) on delete cascade,
    solar_system_id bigint references solar_systems (id),
    position_x      double precision not null,
    position_y      double precision not null,
    alias           text,
    -- A map has at most one home system and at most one rally point (partial unique
    -- indexes below) but any number of pinned systems. Pinned systems are drag-locked and
    -- survive "clear map".
    is_home         boolean not null default false,
    is_rally        boolean not null default false,
    is_pinned       boolean not null default false,
    raised_by_signature_id bigint,
    hangs_off_id    bigint references map_solar_systems (id)
                        on delete cascade deferrable initially deferred,
    created_at      timestamptz not null default now(),

    unique (map_id, solar_system_id),
    -- A node is either a system somebody placed or a hole somebody scanned, never both and
    -- never neither.
    constraint map_solar_systems_ghost_names_its_scan check (
        (solar_system_id is not null
             and raised_by_signature_id is null and hangs_off_id is null)
        or (solar_system_id is null
             and raised_by_signature_id is not null and hangs_off_id is not null)
    ),
    -- Home and rally mean a place you can go; pinning means a place you have decided
    -- matters, which is skipped by every sweep and would let a ghost outlive its hole.
    constraint map_solar_systems_ghost_unmarked
        check (solar_system_id is not null or not (is_home or is_rally or is_pinned))
);

create unique index map_solar_systems_one_home
    on map_solar_systems (map_id) where is_home;
create unique index map_solar_systems_one_rally
    on map_solar_systems (map_id) where is_rally;
create index map_solar_systems_raised_by_idx
    on map_solar_systems (raised_by_signature_id)
    where raised_by_signature_id is not null;

-- Persisted intel: survives a system being removed from the map. A new placement starts
-- as `unknown` (no status icon, neutral border); anything else is an explicit choice.
create table map_solar_system_details (
    id              bigint generated always as identity primary key,
    map_id          bigint not null references maps (id) on delete cascade,
    solar_system_id bigint not null references solar_systems (id),
    status          system_status not null default 'unknown',
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
    type                   connection_type not null,
    mass_status            mass_status,
    time_status            time_status,
    size                   wormhole_size,
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
    "group"                signature_group not null,
    -- The matched catalog type; `name` keeps the raw scanner text when nothing matched.
    signature_type_id      bigint references signature_types (id),
    name                   text,
    size                   wormhole_size,
    mass_status            mass_status,
    time_status            time_status,
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
    subject_type subject_type not null,
    subject_id   bigint not null,
    role         map_role not null,
    -- Access that runs out on its own, for the scout who joined for one operation. `null`
    -- is the ordinary grant: it lasts until it is taken away.
    expires_at   timestamptz,
    created_at   timestamptz not null default now(),

    unique (map_id, subject_id)
);

-- Every role lookup filters on this, and a map's grants are read on nearly every request.
create index map_access_live on map_access (map_id, subject_id)
    where expires_at is null;

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
    route_preference        route_preference not null default 'shorter',
    security_penalty        int not null default 50,
    route_allow_time_status time_status not null default 'critical',
    route_allow_mass_status mass_status not null default 'reduced',
    route_use_evescout      boolean not null default false,
    -- What the jump tracker does on this user's behalf. Off, a jump is mapped straight
    -- away with no signature: the hole still gets built, it just goes unlinked.
    prompt_for_signature    boolean not null default true,
    suggest_alias           boolean not null default true,
    -- Copying without being asked is the kind of thing that steals a clipboard mid-fight.
    copy_bookmark           boolean not null default false,
    -- Which half of the chain the killmails card shows: all / jspace / kspace.
    killmail_filter         killmail_scope not null default 'all',
    -- Hides a finished chain from this user's map list without deleting it for everyone
    -- else. Per-user because one group's dead chain is another's staging map.
    is_archived             boolean not null default false,
    -- When this user finished the map's introduction, the one-time walkthrough of
    -- permissions and preferences. Stamped rather than a flag: when they did it is the
    -- useful half.
    introduction_confirmed_at timestamptz,
    -- Set only on a map that allows it, and only for this viewer.
    layout_override         map_layout,
    -- Quick access: the maps this person keeps in the top bar. Per user, because which
    -- chains you are flying this week is your business.
    is_pinned               boolean not null default false,
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

-- A ghost's scan, declared here because `signatures` is created after the placements that
-- point at it. Deferred for the same reason as its sibling: the undo of a removal puts the
-- placement, the connection and the scan back in whichever order suits it.
alter table map_solar_systems
    add foreign key (raised_by_signature_id) references signatures (id)
        on delete cascade deferrable initially deferred;
