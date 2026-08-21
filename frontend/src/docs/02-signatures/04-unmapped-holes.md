---
title: Unmapped holes
---

# Unmapped holes

A wormhole signature is known before the system behind it is. Turn on **draw unmapped
holes** in the map's settings and every scanned wormhole that leads nowhere yet appears as
a node of its own, hanging off the system it was scanned in.

It is drawn dashed, and labelled with the scanner id, because nobody has been through it.
The point is that a chain can be laid out, named and talked about before anyone flies it:
three unscanned holes off your staging system are three things you can point at in comms
rather than three lines in a signature list.

## What it can and cannot do

| Can                                            | Cannot                                   |
| ---------------------------------------------- | ---------------------------------------- |
| Be dragged, named with an alias, laid out      | Hold signatures or intel of its own      |
| Be counted in the chain                        | Be routed through, or made home or rally |
| Be resolved into the system it turns out to be | Be connected to anything by hand         |

The last one is deliberate: an edge out of an unmapped hole would claim the unknown system
on its far side leads somewhere, which is the one thing nobody knows yet.

## Flying one

Jump it and the app offers to resolve the node you already drew rather than mapping the
same system twice. Any alias you gave the hole comes with it, so a hole you called `2b`
before you flew it is still `2b` afterwards.

If the hole turns out to lead somewhere already on the map — the chain looping back on
itself — the node is merged into the placement that is already there instead of drawing a
second copy.

## When they disappear

An unmapped hole exists because a scan says it does, so it lasts exactly as long as that
scan.

- **Delete the signature** and the node goes with it. This is how you say a hole was never
  really there.
- **Retype the signature** as something other than a wormhole and it goes too.
- **Remove the system it hangs off** and it goes, along with everything else that hung off
  it.

Cutting the node's connection, or unlinking its signature, does **not** get rid of it. The
scan still says the hole is there, so the map draws it again. Delete the signature.

> Turning the setting off removes every unmapped hole from the map at once, and leaves the
> signatures alone. Turning it back on draws them again.
