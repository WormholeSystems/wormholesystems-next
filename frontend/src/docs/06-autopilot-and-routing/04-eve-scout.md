---
title: EVE Scout
---

# EVE Scout

[EVE Scout](https://www.eve-scout.com) is a community project that publicly scans and shares the wormhole connections around **Thera** and **Turnur**. The app can fold that data into your routing.

## What the toggle does

Turn on **Use EVE Scout** in the autopilot settings and the router adds EVE Scout's public, completed wormhole connections to the graph as a third kind of link, alongside [stargates and your map's wormholes](/documentation/autopilot-and-routing/how-routing-works). Because these hubs connect to a constantly-shifting spread of systems across New Eden, they often open up dramatically shorter routes.

The data is pulled from EVE Scout's public API and cached briefly (around five minutes), so newly-reported holes show up within a few minutes and collapsed ones drop out.

## Putting the holes on the map

The EVE Scout panel lists the current holes out of each hub. With **Thera** or **Turnur** selected, the **plus** button in the panel's header puts that hub's whole list on the map: the hub itself if it is not there yet, every system on the far side of a hole, the connections between them, and the signatures on both ends, typed and marked with the mass and lifetime EVE Scout reports.

It only adds what is missing. Systems already on the map stay where they are, a connection that already joins the two is kept, and a signature you scanned yourself is linked rather than replaced (an unclassified one becomes the wormhole; anything you classed as something else is left alone). Run it again as the list changes and only the new holes appear. The whole run is one step in the history, so one undo takes it all back off.

You need the member role to use it.

## When to use it

- **On** — for travel and logistics, where a public shortcut saves jumps.
- **Off** (the default) — when you only want to route through holes your own group controls.

Public connections come and go quickly, so treat them as opportunistic shortcuts rather than guaranteed paths.
