-- Turn the command journal into a history tree with a cursor.
--
-- Undo and redo used to append a new journal row each time, so "where am I in history"
-- had to be guessed from the shape of the rows. That guess said "redo available" forever
-- once you had undone anything. Now each step points at the step that was current when it
-- was applied, and the map remembers which step it is sitting on: undo walks to the
-- parent, redo walks to a child, and neither one writes a new step.
alter table map_events
    -- The step that was current when this one was applied. Null = a root. `set null` (not
    -- cascade) so retention can drop old ancestors without taking their descendants with
    -- them; the oldest surviving step simply becomes a new root and undo stops there.
    add column parent_id bigint references map_events (id) on delete set null,
    -- How to (re)apply this step, recorded when it is undone. Its counterpart `inverse` is
    -- refreshed when it is redone, so both directions reuse the original row ids.
    add column forward jsonb,
    -- Whether this row is a step in the tree at all. Background writers (signature expiry,
    -- jump capture) record for the audit trail but never become undoable steps.
    add column is_step boolean not null default false;

-- The map's cursor. Null means every step has been undone.
alter table maps
    add column head_event_id bigint references map_events (id) on delete set null;

-- Both replaced by the cursor: a step is in effect when it lies on the path from a root
-- to the head, which is derived rather than stored.
alter table map_events
    drop column undone_at,
    drop column reverts_id;

create index map_events_children on map_events (map_id, parent_id) where is_step;
