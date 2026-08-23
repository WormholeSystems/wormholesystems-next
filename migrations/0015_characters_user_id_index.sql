-- Every auth check filters characters by user_id, which was fine while the table held
-- only logged-in people. Killmail ingest shares the table and has grown it to hundreds
-- of thousands of rows, so each of those filters became a full scan. Partial, because
-- ingested characters have no user_id: the index stays a few rows and killmail inserts
-- never touch it.
create index characters_user_id on characters (user_id) where user_id is not null;
