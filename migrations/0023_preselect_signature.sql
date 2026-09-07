-- Whether the jump prompt opens with the likeliest signature already chosen, so working
-- through freshly scanned holes in order is one Enter each. Off by default: a wrong guess
-- confirmed by reflex links the wrong hole, and undoing that costs more than a click.
alter table map_user_settings add column preselect_signature boolean not null default false;
