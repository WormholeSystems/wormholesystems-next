---
title: Self-hosting
category: Contributing & self-hosting
---

# Self-hosting

WormholeSystems is open source, and you can run your own private instance. Everything a
deployment needs is in the one repository, and an operator CLI called **wsctl** does the
work.

> **This is pre-alpha software.** Breaking changes land constantly, there is no promise of
> continued updates, and databases get wiped when it is convenient. Host it to look at it,
> not because a corp is depending on it.

## What you need

- A machine with **Docker**, and a **domain** pointed at it. Certificates are obtained for
  you, so the domain has to resolve before you start.
- Your own **EVE application** from the
  [developer portal](https://developers.eveonline.com/). Logins and tracking run against
  your instance, not the public one, so it needs its own credentials and its own callback
  URL.

## Setting it up

```sh
curl --proto '=https' --tlsv1.2 -sSf https://install-next.wormhole.systems | sh
wsctl setup
```

`wsctl setup` checks the machine first — Docker, disk, ports, DNS — then asks for the
domain, your EVE credentials, and optionally Discord. It writes them to `.env`, generates a
database password, and brings the stack up. The first boot downloads CCP's static data
export, which is a few hundred megabytes and takes a while.

## Looking after it

| Command        | What it does                                                          |
| -------------- | --------------------------------------------------------------------- |
| `wsctl status` | What is running, how far behind the code is, whether the URL answers. |
| `wsctl update` | Takes new code and newer static data, rebuilds, restarts.             |
| `wsctl doctor` | Checks the machine and changes nothing.                               |

Database migrations run when the app boots, so there is no separate step for them.

Nothing backs itself up yet. If you are keeping a chain you care about — see the warning
above about not doing that — take your own dumps.
