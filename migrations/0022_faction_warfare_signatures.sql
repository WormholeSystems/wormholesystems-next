-- ---------------------------------------------------------------------------
-- Factional Warfare sites are a signature group of their own
-- ---------------------------------------------------------------------------
--
-- The scanner labels them `Factional Warfare Site - Combat Site`, and their type names
-- rotate with the war, so they never resolve to a catalogue type. Filed under `combat`
-- they would hide the real combat sites; left unmatched they land in `unknown` and look
-- unscanned. The category itself (id 8) arrives with the next seed.

alter type signature_group add value 'faction_warfare' before 'unknown';
