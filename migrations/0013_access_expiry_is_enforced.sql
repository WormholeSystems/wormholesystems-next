-- Access that has run out should stop counting the moment it does, everywhere.
--
-- `map_access` rows carry an `expires_at`, and only `effective_role` was filtering on it:
-- the four other places that ask "which grants apply to this user" each expanded the
-- character/corporation/alliance union themselves and forgot. An expired grant kept the map
-- in the owner's list, kept it in the Discord picker, and kept that pilot on everyone's
-- presence panel.
--
-- A view rather than a repeated predicate, so the filter is not something a fifth query can
-- leave out. The sweep that deletes them is separate and slower on purpose: it keeps the
-- table from growing, while this keeps the answer right in between two runs of it.
-- The name belongs to the view: what 0006 called `map_access_live` is the partial index
-- for grants that never expire, which is what it has always been.
alter index map_access_live rename to map_access_unexpiring;

create view map_access_live as
    select * from map_access where expires_at is null or expires_at > now();

-- The sweep reads this rather than scanning the whole table.
create index map_access_expiring on map_access (expires_at) where expires_at is not null;
