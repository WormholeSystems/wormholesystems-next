---
title: Using the API
---

# Using the API

Anything the map does, a script can do too. The map screen talks to WormholeSystems over
a JSON API, and every part of it accepts a personal access token in place of being signed
in. A small, stable part of it lives under `/api/v1/` in the same shape the previous
version of WormholeSystems served, so a tool written against that keeps working.

## Getting a token

Open **Settings** from your avatar, then **API tokens**. Give the token a name that says
what it is for, and optionally a lifetime. The token is shown once, right after it is
created. Copy it then; the list afterwards shows only the name, when it was last used and
when it expires.

A token acts as you: it can see the maps you can see and do what your role on each of
them allows, and nothing more. It cannot make or remove other tokens, so if one leaks you
revoke it from the same page and the leak stops there.

## Calling it

Send the token as a bearer header:

```
curl -H "Authorization: Bearer wst_..." https://your.instance/api/v1/maps
```

The reads and writes available under `/api/v1/` are the map list, one map, renaming a
map, and reading, placing, editing and removing a system on a map. Sovereignty for every
claimed system is at `/api/v1/sovereignties` and needs no token. The full reference, with
the shape of each response, is in the repository under `docs/api.md`.

## When a token stops working

A request with an unknown, revoked or expired token is refused outright with `401`, never
quietly treated as a visitor. If a script starts getting that, look at the tokens page: the
one it used is gone or past its date.
