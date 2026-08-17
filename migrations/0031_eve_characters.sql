-- Characters we have had to name, whoever they are.
--
-- Deliberately separate from `characters`, which is the auth table: a row there means this
-- installation holds a token for that pilot, and its `user_id` is not nullable. A killmail
-- victim is neither of those things, but still needs a name next to their portrait.
--
-- `corporations` and `alliances` already play this role for organisations, so this is the
-- missing third of the set rather than a new idea.
create table eve_characters (
    id             bigint primary key,
    name           text not null,
    corporation_id bigint,
    alliance_id    bigint,
    -- When we last asked ESI. Characters change corp, so the entry goes stale.
    updated_at     timestamptz not null default now()
);
