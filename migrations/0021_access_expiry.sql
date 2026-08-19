-- Access that runs out on its own.
--
-- A scout joining for one operation, or a corp given a look at the chain for the weekend,
-- should not need somebody to remember to revoke it afterwards. `null` is the ordinary
-- grant: it lasts until it is taken away.
alter table map_access add column expires_at timestamptz;

-- Every role lookup filters on this, and a map's grants are read on nearly every request.
create index map_access_live on map_access (map_id, subject_id)
    where expires_at is null;
