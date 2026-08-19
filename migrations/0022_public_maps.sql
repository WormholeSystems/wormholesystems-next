-- Maps somebody can look at without an account.
--
-- Two ways in, both read-only. `is_public` puts the map in the open for anyone who has the
-- link to it. `share_token` is a secret in the URL: the map stays private, but whoever
-- holds the token can watch it. Either way the visitor is a viewer and nothing more, and
-- pilots stay hidden, which is Member+ everywhere else too.
alter table maps add column is_public boolean not null default false;
alter table maps add column share_token text unique;
