-- Ids ESI answered 404 for: a closed corporation, a disbanded alliance, a biomassed
-- character. Without this, every resolver pass asks for them again, since an entity
-- that could not be named has no row to say so. Kept apart from the entity tables, whose
-- rows always carry a name.
create table unresolvable_entities (
    kind     text        not null check (kind in ('character', 'corporation', 'alliance')),
    id       bigint      not null,
    noted_at timestamptz not null default now(),
    primary key (kind, id)
);
