-- The access-subject search matches names and tickers with a contains ilike, which no
-- btree can serve: it was scanning every character, corporation and alliance per
-- keystroke once killmail ingest grew those tables. Trigram GIN is the index type made
-- for that filter. Patterns under three characters still scan, which the endpoint's
-- two-character minimum keeps rare.
create extension if not exists pg_trgm;

create index characters_name_trgm on characters using gin (name gin_trgm_ops);
create index corporations_name_trgm on corporations using gin (name gin_trgm_ops);
create index corporations_ticker_trgm on corporations using gin (ticker gin_trgm_ops);
create index alliances_name_trgm on alliances using gin (name gin_trgm_ops);
create index alliances_ticker_trgm on alliances using gin (ticker gin_trgm_ops);

-- Both are filtered on directly and are set-null targets of deletes that happen in
-- routine operation (clean-stale removes connections, alerts get deleted), which
-- otherwise scan the whole child table per delete.
create index signatures_connection_id on signatures (connection_id)
    where connection_id is not null;
create index map_alert_events_alert on map_alert_events (map_alert_id);
