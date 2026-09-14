-- A connection's group is the connection plus at most one signature per endpoint system
-- (see 0007). Nothing enforced that, so two signatures scanned in the same system could
-- both claim one connection, and the sync then made them share its mass and lifetime: an
-- edit to one turned up on the other, while the hole the second signature really was sat
-- unlinked. Keep the first claim on each side, set the rest loose (they keep their own
-- scanned state), and let the database hold the rule from here.
update signatures s
   set connection_id = null, updated_at = now()
 where connection_id is not null
   and exists (
       select 1 from signatures other
       where other.connection_id = s.connection_id
         and other.solar_system_id = s.solar_system_id
         and other.id < s.id
   );

create unique index signatures_one_per_connection_side
    on signatures (connection_id, solar_system_id)
    where connection_id is not null;
