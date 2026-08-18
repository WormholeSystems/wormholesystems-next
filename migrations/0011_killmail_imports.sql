-- Which archived days have already been imported, so a restart resumes where it left off
-- rather than re-downloading a quarter of a year. Days EVE Ref has no archive for are
-- deliberately not recorded: yesterday's file appears some hours late, and skipping it
-- permanently would leave a hole nothing ever fills.
create table killmail_imports (
    day date primary key,
    killmails integer not null,
    imported_at timestamptz not null default now()
);
