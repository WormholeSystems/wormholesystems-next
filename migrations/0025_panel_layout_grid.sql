-- The panel layout becomes a free-form grid over the whole map page, with the canvas as
-- one of the tiles, so a stored arrangement is per-breakpoint positions rather than an
-- order. `panel_order` has nothing left to say once every tile carries its own x/y/w/h.
--
-- Null means "the built-in arrangement", so a map nobody has customised keeps rendering
-- from the defaults in the panel registry.
alter table map_user_settings
    add column layout_breakpoints jsonb,
    drop column panel_order;
