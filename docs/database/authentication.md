# Authentication

EVE SSO identity and ESI tokens: users and their characters, the tokens each
character holds, the scope catalogue, and the short-lived login handshake. Part of
the [database spec](./README.md) — see it for the conventions and goals.

## Authentication (EVE SSO)

Identity comes from **EVE SSO** (OAuth 2.0). A login authenticates a single
**character**, not a user — so the EVE-native identity lives on
[`characters`](#characters). A character authorizes one or more times, each producing
a token with its own scope set, so the tokens live in their own [`tokens`](#tokens)
table; the scopes themselves are catalogued in [`scopes`](#scopes) and joined via
[`token_scopes`](#token_scopes). The JWT also carries a character **owner hash** that
changes if the character is transferred to another EVE account; we store it to detect
transfers (see [`characters`](#characters)). A **`user`** is our own app-side account:
a bag of characters. Which character is *active* is **per session** — a user can be active as a
different character on each device — so it is never stored on the user; a character
may be flagged **preferred** to seed each new session. The first login creates a user
+ its character; logging in with another character while already signed in **links**
that character to the existing user.

We use the **Authorization Code** flow (server-side app holding a client secret). The
short-lived handshake state — the CSRF `state`, plus a PKCE `code_verifier` if we
adopt PKCE — is held in [`oauth_login_flows`](#oauth_login_flows) until the callback.

> **Scopes drive features.** Live pilot-location tracking (the
> [viewer-vs-member line](./access.md#roles--capabilities)) requires the character to
> have granted the location scope. A feature checks for a token of that character
> whose `token_scopes` include the needed scope before calling the matching ESI
> endpoint.

---

## `users`

Our app-side account: a bag of characters. Has **no** EVE-native identity of its own
— that lives on `characters`. The *active* character is **session state**, not stored
here.

| Column           | Type              | Notes                                       |
|------------------|-------------------|---------------------------------------------|
| `id`             | pk                |                                             |
| `last_active_at` | timestamptz, null | last app interaction; gates status polling  |
| `created_at`     | timestamptz       |                                             |

**Invariants & expected behaviour**

- A user has zero or more characters.
- A character belongs to exactly one user; the same EVE character cannot be linked to
  two users at once.
- **Active character is per-session.** A user signed in on two devices can be active
  as a different character on each, so the active character is resolved from the
  session, never from `users`. A new session is seeded from the user's *preferred*
  character (see [`characters`](#characters)).
- `last_active_at` is updated when the user interacts with the app (may be throttled
  rather than written on every request). It gates
  [character status polling](../processes.md#character-status-polling): only users
  active within the last 5 minutes are polled.

> **Open — session store.** Where per-session state (including the active character)
> lives — the web framework's session (cookie/Redis) or a Postgres `sessions` table —
> is the same kind of decision as the [`oauth_login_flows`](#oauth_login_flows) state
> store. We'd add a `sessions` table only if we keep it in Postgres.

---

## `characters`

An EVE character belonging to a user. Carries the EVE identity and the corp/alliance
used for access checks; its SSO tokens live in [`tokens`](#tokens).

| Column           | Type         | Notes                                          |
|------------------|--------------|------------------------------------------------|
| `id`             | pk (bigint)  | EVE character id (from the JWT `sub`)           |
| `user_id`        | fk users     | owning account                                 |
| `name`           | text         | from the JWT `name` claim                      |
| `owner_hash`     | text         | JWT `owner` claim; changes on character transfer |
| `corporation_id` | bigint       | EVE corp id (for access checks)                |
| `alliance_id`    | bigint, null | EVE alliance id (for access checks)            |
| `is_preferred`   | bool         | default `false`; seeds each new session        |
| `updated_at`     | timestamptz  | corp/alliance refreshed from ESI               |

**Invariants & expected behaviour**

- Each character belongs to exactly one user.
- `corporation_id` / `alliance_id` are kept fresh (they drive [access](./access.md))
  — staleness is a real access-correctness risk. They are refreshed **on login** and
  by a **periodic job** — see [affiliation refresh](../processes.md#affiliation-refresh)
  — using the bulk [character affiliation](../esi/characters-affiliation.md) endpoint.
- Re-authenticating a character adds or updates a row in [`tokens`](#tokens), never a
  duplicate character row (id is the EVE character id).
- **The owner hash detects transfers.** On login, if the JWT's `owner_hash` differs
  from the stored value, the character has changed hands (been sold/transferred). The
  login must **not** sign into the user that previously had it; instead the character
  is reassigned to a **new** user and its old link severed, so the new holder can
  never reach the old account's data.
  - *Later concern (not yet specced):* let the previous owner re-claim the character
    onto their old account via an explicit opt-in at login.
- **At most one preferred character per user**, enforced with a *partial* unique
  index: `UNIQUE (user_id) WHERE is_preferred`. (A plain `UNIQUE (user_id,
  is_preferred)` would wrongly also cap *non*-preferred characters at one per user.)
- A new session defaults its active character to the user's preferred one, if any.
- The first character linked to a user becomes its preferred one.

---

## `tokens`

An access/refresh token pair obtained from **one** SSO authorization for a character.
A character may hold several tokens, each granting a different set of scopes.

| Column             | Type              | Notes                                       |
|--------------------|-------------------|---------------------------------------------|
| `id`               | pk                |                                             |
| `character_id`     | fk characters     | the character this token acts for           |
| `access_token`     | text, null        | current ESI access token (JWT), short-lived |
| `token_expires_at` | timestamptz, null | when `access_token` expires                 |
| `refresh_token`    | text              | long-lived; **encrypted at rest**           |
| `created_at`       | timestamptz       |                                             |
| `updated_at`       | timestamptz       | bumped on refresh                           |

**Invariants & expected behaviour**

- Belongs to exactly one character; deleting the character deletes its tokens.
- The **refresh token is the sensitive credential**: encrypted at rest, never logged,
  never sent to the client. The access token is short-lived and renewed from it.
- A token's granted scopes are exactly its [`token_scopes`](#token_scopes) rows, and
  match the JWT `scp` at issuance.
- **Picking a token for an ESI call:** choose a token of the (active) character whose
  scopes include the one the endpoint needs, refreshing it if expired.

> **Open — duplicates.** Do we dedupe tokens by scope set (one token per distinct
> scope set per character, re-auth replacing it), or keep every authorization as its
> own row?

---

## `scopes`

Catalogue of the ESI scopes the application knows about (e.g.
`esi-location.read_location.v1`).

| Column        | Type       | Notes                        |
|---------------|------------|------------------------------|
| `id`          | pk         |                              |
| `name`        | text       | unique; the ESI scope string |
| `description` | text, null | human-readable purpose       |

**Invariants & expected behaviour**

- `name` is unique.
- Reference data: a row says a scope *exists*, independent of any token.

> **Open — seeding.** Seed the full published ESI scope list, only the scopes the app
> requests, or insert-on-first-seen as tokens arrive?

---

## `token_scopes`

Which scopes a token was granted — a many-to-many join between `tokens` and `scopes`.

| Column     | Type      | Notes |
|------------|-----------|-------|
| `token_id` | fk tokens |       |
| `scope_id` | fk scopes |       |

**Invariants & expected behaviour**

- Primary key `(token_id, scope_id)`; a scope appears at most once per token.
- A token's effective scope set is exactly these rows.
- Deleting a token removes its `token_scopes`; a `scope` cannot be deleted while still
  referenced.

---

## `oauth_login_flows`

Short-lived, server-side state for one in-progress SSO handshake — from the authorize
redirect until the callback. Single-use.

| Column          | Type           | Notes                                                 |
|-----------------|----------------|-------------------------------------------------------|
| `state`         | text, pk       | random CSRF token, echoed back by the SSO             |
| `code_verifier` | text, null     | PKCE verifier (only if we use PKCE)                   |
| `link_user_id`  | fk users, null | set when linking a new character to a signed-in user  |
| `redirect_to`   | text, null     | where to send the user after a successful login       |
| `created_at`    | timestamptz    |                                                       |
| `expires_at`    | timestamptz    | short TTL (minutes)                                   |

**Invariants & expected behaviour**

- `state` is unique and **single-use**: consumed (deleted) on a successful callback;
  replaying the same `state` must fail.
- The callback rejects if `state` is unknown or `expires_at` has passed.
- If `link_user_id` is set, the authenticated character is attached to that user;
  otherwise the login resolves or creates a user from the character.

> **Open — state store.** This is classic ephemeral session data. Keep it as a
> Postgres table (simple, transactional) or push it to a cache (Redis)? Modelled as a
> table here; trivial to swap.
