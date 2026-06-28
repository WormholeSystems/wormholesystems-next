//! A single map: the interactive graph. Systems are draggable DOM nodes on a fixed
//! 4000x2000 world; connections are smooth curves in one SVG overlay. The world is panned
//! (middle-mouse / virtual scrollbars) and zoomed (buttons) inside a fixed-height viewport.
//!
//! Realtime: every mutation publishes a [`MapEvent`](crate::maps::MapEvent); the WS bumps
//! `refetch` and the map resource reloads behind a `<Transition>`. Pan/zoom/selection live in
//! component signals *outside* the resource and nodes are keyed by id, so a refetch updates
//! data in place without flicker or losing interaction state.

use std::collections::HashSet;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use web_sys::PointerEvent;

use crate::app::api::{
    add_connection, add_system, clear_map, fetch_map, grid_config, move_system, remove_connection,
    remove_systems, set_alias, set_home, set_occupier, set_pinned, set_status,
    set_connection_status,
};
use crate::app::components::{AllianceImage, CorporationImage, SystemSearchDialog};
use crate::app::GridConfig;
use crate::maps::connection::{AddConnection, RemoveConnection, SetConnectionStatus};
use crate::maps::solar_system::{
    AddSystem, ClearMap, MapSystemView, MoveSystem, RemoveSystems, SetAlias, SetHome, SetOccupier,
    SetPinned, SetStatus, Sovereignty,
};
use crate::maps::{ConnectionType, MapView, MassStatus, SystemStatus, TimeStatus};

/// Fixed node width (px, world space). Height is `2 * grid cell` (see [`GridConfig`]).
const NODE_W: f64 = 176.0;

/// A live position override for the node currently being dragged (world coords).
#[derive(Clone, Copy)]
struct Drag {
    id: i64,
    x: f64,
    y: f64,
}

/// An in-progress connection drag: from this placement to the current cursor (world coords).
#[derive(Clone, Copy)]
struct Linking {
    from: i64,
    x: f64,
    y: f64,
}

/// An open right-click menu, positioned at screen `(x, y)`.
#[derive(Clone)]
struct Menu {
    x: f64,
    y: f64,
    target: MenuTarget,
}

#[derive(Clone)]
enum MenuTarget {
    Map,
    Node(MapSystemView),
    Connection(i64),
}

#[component]
pub fn MapPage() -> impl IntoView {
    let params = use_params_map();
    let map_id = move || {
        params
            .read()
            .get("id")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };

    let status = RwSignal::new(String::new());
    let refetch = RwSignal::new(0u32);

    // Map data + the (static) grid config, both prefetched during SSR and hydrated with data.
    let map = Resource::new(
        move || (map_id(), refetch.get()),
        move |(id, _)| async move { fetch_map(id).await.ok() },
    );
    let grid = Resource::new(
        || (),
        |_| async move { grid_config().await.unwrap_or_default() },
    );

    // Interaction state — owned here, never derived from `map`, so it survives refetch.
    let pan = RwSignal::new((0.0_f64, 0.0_f64));
    let zoom = RwSignal::new(1.0_f64);
    let selected = RwSignal::new(HashSet::<i64>::new());
    let drag = RwSignal::new(None::<Drag>);
    // Optimistic positions held from drop until the server confirms them, so a moved node
    // doesn't flash back to its old spot during the refetch round-trip.
    let pending = RwSignal::new(std::collections::HashMap::<i64, (f64, f64)>::new());
    let linking = RwSignal::new(None::<Linking>);
    let band = RwSignal::new(None::<(f64, f64, f64, f64)>); // world rect: x0,y0,x1,y1
    let menu = RwSignal::new(None::<Menu>);
    let pan_drag = RwSignal::new(None::<(f64, f64, f64, f64)>); // start client + start pan
    let search_open = RwSignal::new(false);
    let link_from = RwSignal::new(None::<i64>); // when set, search adds + connects to this node

    let viewport: NodeRef<leptos::html::Div> = NodeRef::new();
    let gridc = move || grid.get().unwrap_or_default();

    // Render off a plain signal rather than the resource directly: the canvas stays mounted
    // and only its keyed nodes diff on refetch (no whole-subtree remount / flash). We keep the
    // last good value during a refetch (the guard skips a transient `None`).
    let data = RwSignal::new(None::<MapView>);
    Effect::new(move |_| {
        if let Some(Some(mv)) = map.get() {
            data.set(Some(mv));
        }
    });

    // Reconcile optimistic overrides: drop one once the server position matches it (our move
    // landed) or the system is gone. Reads `map` reactively, so it runs on every refetch.
    Effect::new(move |_| {
        if let Some(Some(mv)) = map.get() {
            pending.update(|p| {
                p.retain(|id, (px, py)| {
                    match mv.systems.iter().find(|s| s.id == *id) {
                        Some(s) => {
                            (s.position_x - *px).abs() > 0.5 || (s.position_y - *py).abs() > 0.5
                        }
                        None => false,
                    }
                });
            });
        }
    });

    // Connect the realtime stream once the id is known.
    Effect::new(move |prev: Option<i64>| {
        let id = map_id();
        if id != 0 && prev != Some(id) {
            start_ws(id, refetch);
        }
        id
    });

    // Block the page from scrolling when the wheel is used over the canvas (we don't zoom on
    // wheel — buttons do that). Needs a non-passive listener, which Leptos `on:` can't give us.
    install_wheel_guard(viewport);

    // Screen (client) point -> world coords, accounting for pan + zoom.
    let to_world = move |client_x: f64, client_y: f64| -> (f64, f64) {
        let (left, top, _, _) = viewport_rect(viewport);
        let (px, py) = pan.get_untracked();
        let z = zoom.get_untracked();
        ((client_x - left - px) / z, (client_y - top - py) / z)
    };

    // --- pointer plumbing ---

    let on_pointer_move = move |ev: PointerEvent| {
        let (wx, wy) = to_world(ev.client_x() as f64, ev.client_y() as f64);
        if let Some(d) = drag.get_untracked() {
            drag.set(Some(Drag { x: wx, y: wy, ..d }));
        } else if let Some(l) = linking.get_untracked() {
            linking.set(Some(Linking { x: wx, y: wy, ..l }));
        } else if let Some((x0, y0, _, _)) = band.get_untracked() {
            band.set(Some((x0, y0, wx, wy)));
        } else if let Some((cx, cy, p0x, p0y)) = pan_drag.get_untracked() {
            pan.set((p0x + ev.client_x() as f64 - cx, p0y + ev.client_y() as f64 - cy));
        }
    };

    let on_pointer_up = move |ev: PointerEvent| {
        let id = map_id();
        // Finish a node drag → persist the new position.
        if let Some(d) = drag.get_untracked() {
            // Hand the position from the live drag to the optimistic override before clearing
            // the drag, so the node stays put across the refetch instead of flashing back.
            pending.update(|p| {
                p.insert(d.id, (d.x, d.y));
            });
            drag.set(None);
            let cmd = MoveSystem {
                map_id: id,
                map_solar_system_id: d.id,
                x: d.x,
                y: d.y,
            };
            run(status, refetch, "move", async move {
                move_system(cmd).await
            });
        }
        // Finish a connection drag → connect if released over a node.
        if let Some(l) = linking.get_untracked() {
            linking.set(None);
            let (wx, wy) = to_world(ev.client_x() as f64, ev.client_y() as f64);
            if let Some(target) = map
                .get_untracked()
                .flatten()
                .and_then(|mv| node_at(&mv.systems, wx, wy, gridc()))
                && target != l.from
            {
                let cmd = AddConnection {
                    map_id: id,
                    from_system: l.from,
                    to_system: target,
                    kind: ConnectionType::Wormhole,
                };
                run(status, refetch, "connect", async move {
                    add_connection(cmd).await.map(|_| ())
                });
            }
        }
        // Finish a rubber-band → select enclosed nodes.
        if let Some((x0, y0, x1, y1)) = band.get_untracked() {
            band.set(None);
            if let Some(mv) = map.get_untracked().flatten() {
                let (lo_x, hi_x) = (x0.min(x1), x0.max(x1));
                let (lo_y, hi_y) = (y0.min(y1), y0.max(y1));
                let h = 2.0 * gridc().cell_size;
                let hit: HashSet<i64> = mv
                    .systems
                    .iter()
                    .filter(|s| {
                        let (cx, cy) = (s.position_x + NODE_W / 2.0, s.position_y + h / 2.0);
                        cx >= lo_x && cx <= hi_x && cy >= lo_y && cy <= hi_y
                    })
                    .map(|s| s.id)
                    .collect();
                selected.set(hit);
            }
        }
        pan_drag.set(None);
    };

    // Background press: middle = pan, left on empty = rubber-band (and clear selection/menu).
    let on_background_down = move |ev: PointerEvent| {
        menu.set(None);
        if let Some(el) = viewport.get_untracked() {
            let _ = el.set_pointer_capture(ev.pointer_id());
        }
        if ev.button() == 1 {
            ev.prevent_default();
            let (px, py) = pan.get_untracked();
            pan_drag.set(Some((ev.client_x() as f64, ev.client_y() as f64, px, py)));
        } else if ev.button() == 0 {
            selected.set(HashSet::new());
            let (wx, wy) = to_world(ev.client_x() as f64, ev.client_y() as f64);
            band.set(Some((wx, wy, wx, wy)));
        }
    };

    // Delete key removes the current selection (bulk).
    let on_key = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Delete" || ev.key() == "Backspace" {
            let ids: Vec<i64> = selected.get_untracked().into_iter().collect();
            if !ids.is_empty() {
                ev.prevent_default();
                selected.set(HashSet::new());
                let cmd = RemoveSystems {
                    map_id: map_id(),
                    map_solar_system_ids: ids,
                };
                run(status, refetch, "remove", async move {
                    remove_systems(cmd).await
                });
            }
        }
    };

    // --- search dialog (Add system / Add connection) ---
    let on_search_select = move |solar_system_id: i64| {
        let id = map_id();
        let from = link_from.get_untracked();
        link_from.set(None);
        let mv = map.get_untracked().flatten();
        // Already placed?
        let existing = mv
            .as_ref()
            .and_then(|m| m.systems.iter().find(|s| s.solar_system_id == solar_system_id))
            .map(|s| s.id);
        let (cx, cy) = center_world(pan.get_untracked(), zoom.get_untracked(), viewport, gridc());
        run(status, refetch, "add", async move {
            let placement = match existing {
                Some(pid) => pid,
                None => {
                    add_system(AddSystem {
                        map_id: id,
                        solar_system_id,
                        x: cx,
                        y: cy,
                        alias: None,
                    })
                    .await?
                    .id
                }
            };
            if let Some(from) = from
                && from != placement
            {
                add_connection(AddConnection {
                    map_id: id,
                    from_system: from,
                    to_system: placement,
                    kind: ConnectionType::Wormhole,
                })
                .await?;
            }
            Ok(())
        });
    };

    let zoom_by = move |factor: f64| {
        let z = zoom.get_untracked();
        let nz = (z * factor).clamp(0.25, 3.0);
        // Keep the viewport center fixed while zooming.
        let (px, py) = pan.get_untracked();
        let (w, h) = viewport_size(viewport);
        let (cx, cy) = (w / 2.0, h / 2.0);
        let wx = (cx - px) / z;
        let wy = (cy - py) / z;
        pan.set((cx - wx * nz, cy - wy * nz));
        zoom.set(nz);
    };

    view! {
        <div class="flex items-center justify-between">
            <a
                href="/maps"
                class="text-sm text-muted-foreground transition-colors hover:text-foreground"
            >
                "← Maps"
            </a>
            <span class="font-mono text-xs text-muted-foreground">{move || status.get()}</span>
        </div>

        <SystemSearchDialog open=search_open on_select=Callback::new(on_search_select) />

        <div
            node_ref=viewport
            tabindex="0"
            class="relative mt-3 w-full overflow-hidden border border-border bg-zinc-950 outline-none select-none"
            style:height=move || format!("{}px", gridc().viewport_height)
            on:pointerdown=on_background_down
            on:pointermove=on_pointer_move
            on:pointerup=on_pointer_up
            on:keydown=on_key
            on:contextmenu=move |ev: web_sys::MouseEvent| {
                ev.prevent_default();
                menu.set(Some(Menu {
                    x: ev.client_x() as f64,
                    y: ev.client_y() as f64,
                    target: MenuTarget::Map,
                }));
            }
        >
            // The transformed world: nodes + the connection overlay scale & pan together.
            <div
                class="absolute top-0 left-0 origin-top-left"
                style:width=move || format!("{}px", gridc().world_width)
                style:height=move || format!("{}px", gridc().world_height)
                style:background-image=move || grid_background(gridc().cell_size)
                style:background-size=move || {
                    format!("{0}px {0}px", gridc().cell_size)
                }
                style:transform=move || {
                    let (px, py) = pan.get();
                    format!("translate({px}px, {py}px) scale({})", zoom.get())
                }
            >
                <WorldContent
                    data=data grid=Signal::derive(gridc) map_id=Signal::derive(map_id)
                    status=status refetch=refetch
                    selected=selected drag=drag pending=pending linking=linking band=band menu=menu
                    search_open=search_open link_from=link_from
                    on_node_down=Callback::new(move |(ev, s): (PointerEvent, MapSystemView)| {
                        handle_node_down(ev, s, viewport, pan, zoom, drag, menu, selected);
                    })
                    on_link_down=Callback::new(move |(ev, id): (PointerEvent, i64)| {
                        ev.stop_propagation();
                        if let Some(el) = viewport.get_untracked() {
                            let _ = el.set_pointer_capture(ev.pointer_id());
                        }
                        let (wx, wy) = world_of(ev, viewport, pan, zoom);
                        linking.set(Some(Linking { from: id, x: wx, y: wy }));
                    })
                />
            </div>

            // Virtual scrollbars (proportional thumbs reflecting viewport over the world).
            <Scrollbars pan=pan zoom=zoom viewport=viewport grid=Signal::derive(gridc) />

            // Zoom controls.
            <div class="absolute bottom-3 right-3 flex flex-col overflow-hidden border border-border bg-card">
                <button
                    class="px-2.5 py-1 text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
                    on:click=move |_| zoom_by(1.2)
                >
                    "+"
                </button>
                <button
                    class="border-t border-border px-2.5 py-1 text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
                    on:click=move |_| zoom_by(1.0 / 1.2)
                >
                    "−"
                </button>
            </div>

            // Context menu.
            {move || menu.get().map(|m| view! {
                <ContextMenu
                    menu=m map_id=Signal::derive(map_id) status=status refetch=refetch
                    close=Callback::new(move |_| menu.set(None))
                    search_open=search_open link_from=link_from
                />
            })}
        </div>
    }
}

/// The data-dependent layer: connection overlay + nodes. Re-rendered on refetch (keyed), but
/// nested inside the persistent transformed world so pan/zoom never reset.
#[component]
fn WorldContent(
    data: RwSignal<Option<MapView>>,
    grid: Signal<GridConfig>,
    map_id: Signal<i64>,
    status: RwSignal<String>,
    refetch: RwSignal<u32>,
    selected: RwSignal<HashSet<i64>>,
    drag: RwSignal<Option<Drag>>,
    pending: RwSignal<std::collections::HashMap<i64, (f64, f64)>>,
    linking: RwSignal<Option<Linking>>,
    band: RwSignal<Option<(f64, f64, f64, f64)>>,
    menu: RwSignal<Option<Menu>>,
    search_open: RwSignal<bool>,
    link_from: RwSignal<Option<i64>>,
    on_node_down: Callback<(PointerEvent, MapSystemView)>,
    on_link_down: Callback<(PointerEvent, i64)>,
) -> impl IntoView {
    let _ = (map_id, status, refetch, search_open, link_from);
    let node_h = move || 2.0 * grid.get().cell_size;

    let systems = move || data.get().map(|mv| mv.systems).unwrap_or_default();
    let connections = move || data.get().map(|mv| mv.connections).unwrap_or_default();

    // Position lookup that respects an in-progress drag override. A `Memo` (Copy) so it can be
    // read from the edge overlay and every node child without move headaches.
    let positions = Memo::new(move |_| {
        let d = drag.get();
        let pend = pending.get();
        systems()
            .iter()
            .map(|s| {
                // Live drag wins; then an optimistic override; then the server position.
                let (x, y) = match d {
                    Some(dd) if dd.id == s.id => (dd.x, dd.y),
                    _ => pend
                        .get(&s.id)
                        .copied()
                        .unwrap_or((s.position_x, s.position_y)),
                };
                (s.id, (x, y))
            })
            .collect::<std::collections::HashMap<i64, (f64, f64)>>()
    });

    view! {
        <svg
            class="absolute top-0 left-0 overflow-visible"
            style:width=move || format!("{}px", grid.get().world_width)
            style:height=move || format!("{}px", grid.get().world_height)
        >
            // Edges.
            {move || {
                let pos = positions.get();
                let node_h = node_h();
                connections()
                    .iter()
                    .filter_map(|c| {
                        let (ax, ay) = pos.get(&c.from_system).copied()?;
                        let (bx, by) = pos.get(&c.to_system).copied()?;
                        let (sx, sy, ex, ey) = anchors(ax, ay, bx, by, node_h);
                        let d = bezier(sx, sy, ex, ey);
                        let color = edge_color(c.mass_status, c.time_status);
                        let cid = c.id;
                        Some(view! {
                            <g class="group">
                                // Wide invisible hit area for easy click / right-click.
                                <path
                                    d=d.clone() fill="none" stroke="transparent" stroke-width="14"
                                    style="cursor:pointer"
                                    on:contextmenu=move |ev: web_sys::MouseEvent| {
                                        ev.prevent_default();
                                        ev.stop_propagation();
                                        menu.set(Some(Menu {
                                            x: ev.client_x() as f64,
                                            y: ev.client_y() as f64,
                                            target: MenuTarget::Connection(cid),
                                        }));
                                    }
                                />
                                <path
                                    d=d fill="none" stroke=color stroke-width="2"
                                    class="transition-[stroke-width] group-hover:[stroke-width:4]"
                                />
                            </g>
                        })
                    })
                    .collect_view()
            }}

            // Live connection-drag preview.
            {move || linking.get().and_then(|l| {
                let pos = positions.get();
                let (ax, ay) = pos.get(&l.from).copied()?;
                let (sx, sy, ex, ey) = anchors(ax, ay, l.x, l.y, node_h());
                Some(view! {
                    <path d=bezier(sx, sy, ex, ey) fill="none" stroke="#9ca3af"
                        stroke-width="2" stroke-dasharray="5 4" />
                })
            })}

            // Rubber-band rectangle.
            {move || band.get().map(|(x0, y0, x1, y1)| {
                let (x, y) = (x0.min(x1), y0.min(y1));
                let (w, h) = ((x1 - x0).abs(), (y1 - y0).abs());
                view! {
                    <rect x=x y=y width=w height=h fill="rgba(99,102,241,0.12)"
                        stroke="#6366f1" stroke-width="1" />
                }
            })}
        </svg>

        // Nodes (DOM, keyed by id so refetch diffs in place).
        <For
            each=move || systems()
            key=|s| s.id
            children=move |s| {
                let id = s.id;
                let pos = Memo::new(move |_| {
                    positions.get().get(&id).copied().unwrap_or((0.0, 0.0))
                });
                let is_selected = move || selected.get().contains(&id);
                let s_for_down = s.clone();
                let s_node = s.clone();
                view! {
                    <SystemNode
                        s=s_node node_h=node_h() selected=Signal::derive(is_selected)
                        pos=pos.into()
                        on_down=Callback::new(move |ev: PointerEvent| {
                            on_node_down.run((ev, s_for_down.clone()));
                        })
                        on_link=Callback::new(move |ev: PointerEvent| on_link_down.run((ev, id)))
                        on_menu=Callback::new(move |(ev, s): (web_sys::MouseEvent, MapSystemView)| {
                            ev.prevent_default();
                            ev.stop_propagation();
                            menu.set(Some(Menu {
                                x: ev.client_x() as f64,
                                y: ev.client_y() as f64,
                                target: MenuTarget::Node(s),
                            }));
                        })
                    />
                }
            }
        />
    }
    .into_any()
}

/// One placed system, positioned absolutely in world space.
#[component]
fn SystemNode(
    s: MapSystemView,
    node_h: f64,
    selected: Signal<bool>,
    pos: Signal<(f64, f64)>,
    on_down: Callback<PointerEvent>,
    on_link: Callback<PointerEvent>,
    on_menu: Callback<(web_sys::MouseEvent, MapSystemView)>,
) -> impl IntoView {
    let alias = s.alias.clone();
    let occupier = s.occupying_group.clone();
    let name = s.name.clone();
    let class = class_label(s.wormhole_class_id, s.security_status);
    let class_color = security_color(s.wormhole_class_id, s.security_status);
    let statics_or_region = if s.statics.is_empty() {
        s.region.clone()
    } else {
        s.statics
            .iter()
            .map(|st| format!("→{}", class_label(st.dest_class, 0.0)))
            .collect::<Vec<_>>()
            .join("  ")
    };
    let sov = s.sovereignty.clone();
    let effect = s.effect_name.clone();
    let wclass = s.wormhole_class_id.unwrap_or(0);
    let pinned = s.is_pinned;
    let home = s.is_home;
    let s_menu = s.clone();

    view! {
        <div
            class="group absolute flex flex-col border bg-card px-2 py-1 text-xs shadow-sm"
            class=("border-primary", move || selected.get())
            class=("border-border", move || !selected.get())
            class=("ring-1", move || home)
            class=("ring-amber-500", move || home)
            style:width=format!("{NODE_W}px")
            style:height=format!("{node_h}px")
            style:left=move || format!("{}px", pos.get().0)
            style:top=move || format!("{}px", pos.get().1)
            on:contextmenu=move |ev: web_sys::MouseEvent| on_menu.run((ev, s_menu.clone()))
        >
            // Drag handle (top), hover-only, hidden when pinned.
            {(!pinned).then(|| view! {
                <div
                    class="absolute -top-2 left-1/2 hidden h-3 w-8 -translate-x-1/2 cursor-grab rounded-sm bg-muted-foreground/60 group-hover:block"
                    on:pointerdown=move |ev: PointerEvent| { ev.stop_propagation(); on_down.run(ev); }
                />
            })}
            // Connection handle (right edge), hover-only.
            <div
                class="absolute top-1/2 -right-2 hidden h-3 w-3 -translate-y-1/2 cursor-crosshair rounded-full bg-primary group-hover:block"
                on:pointerdown=move |ev: PointerEvent| on_link.run(ev)
            />

            // Name line: [alias] name [occupier].
            <div class="flex items-center gap-1 truncate font-medium text-foreground">
                {alias.map(|a| view! { <span class="text-primary">{a}</span> })}
                <span class="truncate">{name}</span>
                {occupier.map(|o| view! { <span class="text-muted-foreground">{o}</span> })}
                {pinned.then(|| view! { <span class="text-amber-500" title="pinned">"📌"</span> })}
            </div>

            // Class + security + sovereignty.
            <div class="flex items-center gap-1 truncate text-muted-foreground">
                <span style:color=class_color>{class}</span>
                {sov.map(|sv| sovereignty_view(sv))}
            </div>

            // Statics or region.
            <div class="truncate text-[10px] text-muted-foreground">{statics_or_region}</div>

            // Effect indicator + popover.
            {effect.map(|name| view! {
                <EffectBadge name=name wormhole_class_id=wclass />
            })}
        </div>
    }
}

fn sovereignty_view(sov: Sovereignty) -> impl IntoView {
    let img = "h-3.5 w-3.5 shrink-0";
    match sov {
        Sovereignty::Alliance { id, name, ticker } => view! {
            <span class="flex items-center gap-1 truncate" title=name>
                <AllianceImage id=id class=img />
                <span class="truncate">{ticker}</span>
            </span>
        }
        .into_any(),
        Sovereignty::Corporation { id, name, ticker } => view! {
            <span class="flex items-center gap-1 truncate" title=name>
                <CorporationImage id=id class=img />
                <span class="truncate">{ticker}</span>
            </span>
        }
        .into_any(),
        Sovereignty::Faction { id: _, name } => view! {
            <span class="truncate">{name}</span>
        }
        .into_any(),
    }
}

/// The effect icon plus a hover/focus popover listing the buffs/debuffs at this class.
#[component]
fn EffectBadge(name: String, wormhole_class_id: i32) -> impl IntoView {
    let label = name.clone();
    let mods = Resource::new(
        move || (name.clone(), wormhole_class_id),
        |(name, class)| async move {
            crate::app::api::effect_modifiers(name, class)
                .await
                .unwrap_or_default()
        },
    );
    view! {
        <div class="group/eff absolute top-1 right-1">
            <span class="block h-2.5 w-2.5 cursor-help rounded-full bg-fuchsia-500" tabindex="0" />
            <div class="invisible absolute right-0 z-20 mt-1 w-48 border border-border bg-popover p-2 text-[11px] text-popover-foreground shadow-md group-hover/eff:visible group-focus-within/eff:visible">
                <div class="font-medium">{label}</div>
                <Transition>
                    {move || Suspend::new(async move {
                        mods.await
                            .into_iter()
                            .map(|m| view! {
                                <div class="flex justify-between gap-2">
                                    <span class="text-muted-foreground">{m.stat}</span>
                                    <span>{m.value}</span>
                                </div>
                            })
                            .collect_view()
                    })}
                </Transition>
            </div>
        </div>
    }
}

/// Custom proportional scrollbars: the thumb size/position is the viewport-over-world ratio at
/// the current zoom; dragging it pans.
#[component]
fn Scrollbars(
    pan: RwSignal<(f64, f64)>,
    zoom: RwSignal<f64>,
    viewport: NodeRef<leptos::html::Div>,
    grid: Signal<GridConfig>,
) -> impl IntoView {
    // Visible world span = viewport_size / zoom. Thumb fraction = visible / world.
    let h_thumb = move || {
        let g = grid.get();
        let (vw, _) = viewport_size(viewport);
        let visible = vw / zoom.get();
        let frac = (visible / g.world_width).min(1.0);
        let start = (-pan.get().0 / zoom.get() / g.world_width).clamp(0.0, 1.0 - frac);
        (start * 100.0, frac * 100.0)
    };
    let v_thumb = move || {
        let g = grid.get();
        let (_, vh) = viewport_size(viewport);
        let visible = vh / zoom.get();
        let frac = (visible / g.world_height).min(1.0);
        let start = (-pan.get().1 / zoom.get() / g.world_height).clamp(0.0, 1.0 - frac);
        (start * 100.0, frac * 100.0)
    };
    view! {
        <div class="pointer-events-none absolute inset-x-1 bottom-1 h-1.5">
            <div
                class="absolute h-full rounded-full bg-muted-foreground/40"
                style:left=move || format!("{}%", h_thumb().0)
                style:width=move || format!("{}%", h_thumb().1)
            />
        </div>
        <div class="pointer-events-none absolute inset-y-1 right-1 w-1.5">
            <div
                class="absolute w-full rounded-full bg-muted-foreground/40"
                style:top=move || format!("{}%", v_thumb().0)
                style:height=move || format!("{}%", v_thumb().1)
            />
        </div>
    }
}

/// The right-click menu. Renders the option set for whatever was clicked.
#[component]
fn ContextMenu(
    menu: Menu,
    map_id: Signal<i64>,
    status: RwSignal<String>,
    refetch: RwSignal<u32>,
    close: Callback<()>,
    search_open: RwSignal<bool>,
    link_from: RwSignal<Option<i64>>,
) -> impl IntoView {
    let item = "block w-full px-3 py-1 text-left text-xs text-foreground hover:bg-accent";
    let body = match menu.target.clone() {
        MenuTarget::Map => {
            let add = move |_| {
                link_from.set(None);
                search_open.set(true);
                close.run(());
            };
            let clear = move |_| {
                let cmd = ClearMap { map_id: map_id.get_untracked() };
                run(status, refetch, "clear map", async move { clear_map(cmd).await });
                close.run(());
            };
            view! {
                <button class=item on:click=add>"Add solar system"</button>
                <button class=item on:click=clear>"Clear map"</button>
            }
            .into_any()
        }
        MenuTarget::Node(s) => {
            let id = s.id;
            let mid = map_id;
            let connect = move |_| {
                link_from.set(Some(id));
                search_open.set(true);
                close.run(());
            };
            let rename = move |_| {
                if let Some(alias) = prompt("Alias (blank to clear):") {
                    let cmd = SetAlias {
                        map_id: mid.get_untracked(),
                        map_solar_system_id: id,
                        alias: (!alias.is_empty()).then_some(alias),
                    };
                    run(status, refetch, "alias", async move { set_alias(cmd).await });
                }
                close.run(());
            };
            let occupier = move |_| {
                if let Some(occ) = prompt("Occupier (blank to clear):") {
                    let cmd = SetOccupier {
                        map_id: mid.get_untracked(),
                        map_solar_system_id: id,
                        occupier: (!occ.is_empty()).then_some(occ),
                    };
                    run(status, refetch, "occupier", async move { set_occupier(cmd).await });
                }
                close.run(());
            };
            let home = s.is_home;
            let toggle_home = move |_| {
                let cmd = SetHome { map_id: mid.get_untracked(), map_solar_system_id: id, value: !home };
                run(status, refetch, "home", async move { set_home(cmd).await });
                close.run(());
            };
            let pinned = s.is_pinned;
            let toggle_pin = move |_| {
                let cmd = SetPinned { map_id: mid.get_untracked(), map_solar_system_id: id, value: !pinned };
                run(status, refetch, "pin", async move { set_pinned(cmd).await });
                close.run(());
            };
            let remove = move |_| {
                let cmd = RemoveSystems {
                    map_id: mid.get_untracked(),
                    map_solar_system_ids: vec![id],
                };
                run(status, refetch, "remove", async move { remove_systems(cmd).await });
                close.run(());
            };
            view! {
                <button class=item on:click=connect>"Add connection"</button>
                <button class=item on:click=rename>"Rename alias"</button>
                <button class=item on:click=occupier>"Set occupier"</button>
                <StatusItems id=id map_id=mid status=status refetch=refetch close=close />
                <button class=item on:click=toggle_home>
                    {if home { "Unset home" } else { "Set as home" }}
                </button>
                <button class=item on:click=toggle_pin>
                    {if pinned { "Unpin" } else { "Pin" }}
                </button>
                <button class=item on:click=remove>"Remove system"</button>
            }
            .into_any()
        }
        MenuTarget::Connection(cid) => {
            let mid = map_id;
            let set_kind = move |kind: ConnectionType| {
                let cmd = SetConnectionStatus {
                    map_id: mid.get_untracked(),
                    connection_id: cid,
                    kind: Some(kind),
                    ..Default::default()
                };
                run(status, refetch, "conn type", async move {
                    set_connection_status(cmd).await.map(|_| ())
                });
                close.run(());
            };
            let mark = move |mass: Option<MassStatus>, time: Option<TimeStatus>| {
                let cmd = SetConnectionStatus {
                    map_id: mid.get_untracked(),
                    connection_id: cid,
                    mass_status: Some(mass),
                    time_status: Some(time),
                    ..Default::default()
                };
                run(status, refetch, "conn status", async move {
                    set_connection_status(cmd).await.map(|_| ())
                });
                close.run(());
            };
            let remove = move |_| {
                let cmd = RemoveConnection { map_id: mid.get_untracked(), connection_id: cid };
                run(status, refetch, "del conn", async move { remove_connection(cmd).await });
                close.run(());
            };
            view! {
                <button class=item on:click=move |_| set_kind(ConnectionType::Wormhole)>"Type: wormhole"</button>
                <button class=item on:click=move |_| set_kind(ConnectionType::Stargate)>"Type: stargate"</button>
                <div class="my-0.5 border-t border-border" />
                <button class=item on:click=move |_| mark(Some(MassStatus::Reduced), None)>"Mass: reduced"</button>
                <button class=item on:click=move |_| mark(Some(MassStatus::Critical), None)>"Mass: critical"</button>
                <button class=item on:click=move |_| mark(None, Some(TimeStatus::Eol))>"Time: EOL"</button>
                <button class=item on:click=move |_| mark(Some(MassStatus::Stable), Some(TimeStatus::Stable))>"Reset stable"</button>
                <div class="my-0.5 border-t border-border" />
                <button class=item on:click=remove>"Delete connection"</button>
            }
            .into_any()
        }
    };
    view! {
        <div
            class="fixed z-30 min-w-40 border border-border bg-popover py-1 shadow-md"
            style:left=format!("{}px", menu.x)
            style:top=format!("{}px", menu.y)
        >
            {body}
        </div>
    }
}

/// The "Set status" sub-buttons (one per [`SystemStatus`]).
#[component]
fn StatusItems(
    id: i64,
    map_id: Signal<i64>,
    status: RwSignal<String>,
    refetch: RwSignal<u32>,
    close: Callback<()>,
) -> impl IntoView {
    let item = "block w-full px-3 py-1 text-left text-xs text-muted-foreground hover:bg-accent hover:text-foreground";
    let options = [
        SystemStatus::Unscanned,
        SystemStatus::Scanned,
        SystemStatus::Occupied,
        SystemStatus::Friendly,
        SystemStatus::Hostile,
        SystemStatus::Unknown,
    ];
    options
        .into_iter()
        .map(move |st| {
            let set = move |_| {
                let cmd = SetStatus {
                    map_id: map_id.get_untracked(),
                    map_solar_system_id: id,
                    status: st,
                };
                run(status, refetch, "status", async move { set_status(cmd).await });
                close.run(());
            };
            view! { <button class=item on:click=set>{format!("Status: {}", st.as_str())}</button> }
        })
        .collect_view()
}

// --- helpers ---

/// Press on a node body: select it (replacing the selection) and start dragging unless pinned.
fn handle_node_down(
    ev: PointerEvent,
    s: MapSystemView,
    viewport: NodeRef<leptos::html::Div>,
    pan: RwSignal<(f64, f64)>,
    zoom: RwSignal<f64>,
    drag: RwSignal<Option<Drag>>,
    menu: RwSignal<Option<Menu>>,
    selected: RwSignal<HashSet<i64>>,
) {
    if ev.button() != 0 {
        return;
    }
    ev.stop_propagation();
    menu.set(None);
    if !selected.get_untracked().contains(&s.id) {
        selected.set(HashSet::from([s.id]));
    }
    if s.is_pinned {
        return;
    }
    if let Some(el) = viewport.get_untracked() {
        let _ = el.set_pointer_capture(ev.pointer_id());
    }
    let (wx, wy) = world_of(ev, viewport, pan, zoom);
    let _ = (wx, wy);
    drag.set(Some(Drag {
        id: s.id,
        x: s.position_x,
        y: s.position_y,
    }));
}

fn world_of(
    ev: PointerEvent,
    viewport: NodeRef<leptos::html::Div>,
    pan: RwSignal<(f64, f64)>,
    zoom: RwSignal<f64>,
) -> (f64, f64) {
    let (left, top, _, _) = viewport_rect(viewport);
    let (px, py) = pan.get_untracked();
    let z = zoom.get_untracked();
    (
        (ev.client_x() as f64 - left - px) / z,
        (ev.client_y() as f64 - top - py) / z,
    )
}

fn viewport_size(viewport: NodeRef<leptos::html::Div>) -> (f64, f64) {
    let (_, _, w, h) = viewport_rect(viewport);
    (w, h)
}

/// The viewport's `(left, top, width, height)` in client coords. Hydrate-only (the DOM rect);
/// the SSR stub returns a sensible default since geometry is never used during SSR.
#[cfg(feature = "hydrate")]
fn viewport_rect(viewport: NodeRef<leptos::html::Div>) -> (f64, f64, f64, f64) {
    viewport
        .get_untracked()
        .map(|el| {
            let r = el.get_bounding_client_rect();
            (r.left(), r.top(), r.width(), r.height())
        })
        .unwrap_or((0.0, 0.0, 1200.0, 1400.0))
}

#[cfg(not(feature = "hydrate"))]
fn viewport_rect(_viewport: NodeRef<leptos::html::Div>) -> (f64, f64, f64, f64) {
    (0.0, 0.0, 1200.0, 1400.0)
}

/// World coords of the viewport center (where a freshly-added system lands).
fn center_world(
    pan: (f64, f64),
    zoom: f64,
    viewport: NodeRef<leptos::html::Div>,
    _g: GridConfig,
) -> (f64, f64) {
    let (w, h) = viewport_size(viewport);
    ((w / 2.0 - pan.0) / zoom, (h / 2.0 - pan.1) / zoom)
}

/// The placement id whose node bounds contain the world point, if any.
fn node_at(systems: &[MapSystemView], wx: f64, wy: f64, g: GridConfig) -> Option<i64> {
    let h = 2.0 * g.cell_size;
    systems
        .iter()
        .find(|s| {
            wx >= s.position_x
                && wx <= s.position_x + NODE_W
                && wy >= s.position_y
                && wy <= s.position_y + h
        })
        .map(|s| s.id)
}

/// Box-edge anchor points for an edge between two node top-left corners.
fn anchors(ax: f64, ay: f64, bx: f64, by: f64, node_h: f64) -> (f64, f64, f64, f64) {
    let (acx, acy) = (ax + NODE_W / 2.0, ay + node_h / 2.0);
    let (bcx, bcy) = (bx + NODE_W / 2.0, by + node_h / 2.0);
    let (sx, sy) = box_edge(acx, acy, bcx, bcy, NODE_W / 2.0, node_h / 2.0);
    let (ex, ey) = box_edge(bcx, bcy, acx, acy, NODE_W / 2.0, node_h / 2.0);
    (sx, sy, ex, ey)
}

/// Where the segment from a box center toward `(tx, ty)` crosses the box border.
fn box_edge(cx: f64, cy: f64, tx: f64, ty: f64, hw: f64, hh: f64) -> (f64, f64) {
    let dx = tx - cx;
    let dy = ty - cy;
    if dx == 0.0 && dy == 0.0 {
        return (cx, cy);
    }
    let scale = (hw / dx.abs()).min(hh / dy.abs().max(f64::EPSILON));
    let scale = if dx.abs() < f64::EPSILON {
        hh / dy.abs()
    } else if dy.abs() < f64::EPSILON {
        hw / dx.abs()
    } else {
        scale
    };
    (cx + dx * scale, cy + dy * scale)
}

/// A horizontal-tangent cubic bézier between two points.
fn bezier(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    let ox = ((x2 - x1).abs() * 0.4).max(40.0);
    format!("M {x1} {y1} C {} {y1}, {} {y2}, {x2} {y2}", x1 + ox, x2 - ox)
}

fn grid_background(cell: f64) -> String {
    let _ = cell;
    "linear-gradient(to right, rgba(255,255,255,0.04) 1px, transparent 1px), \
     linear-gradient(to bottom, rgba(255,255,255,0.04) 1px, transparent 1px)"
        .to_string()
}

/// `wormhole_class_id` → short label. K-space (no class) falls back to a security band.
fn class_label(wclass: Option<i32>, security: f64) -> String {
    match wclass {
        Some(c @ 1..=6) => format!("C{c}"),
        Some(7) => "HS".into(),
        Some(8) => "LS".into(),
        Some(9) => "NS".into(),
        Some(12) => "Thera".into(),
        Some(13) => "C13".into(),
        Some(c) => format!("C{c}"),
        None => security_band(security).into(),
    }
}

fn security_band(security: f64) -> &'static str {
    if security >= 0.45 {
        "HS"
    } else if security > 0.0 {
        "LS"
    } else {
        "NS"
    }
}

fn security_color(wclass: Option<i32>, security: f64) -> &'static str {
    match wclass {
        Some(7) | None if security >= 0.45 => "#34d399",
        Some(8) => "#f59e0b",
        None if security > 0.0 => "#f59e0b",
        Some(9) | None => "#ef4444",
        Some(_) => "#60a5fa", // wormhole space
    }
}

fn edge_color(mass: Option<MassStatus>, time: Option<TimeStatus>) -> &'static str {
    if mass == Some(MassStatus::Critical) || time == Some(TimeStatus::Critical) {
        "#ef4444"
    } else if mass == Some(MassStatus::Reduced) || time == Some(TimeStatus::Eol) {
        "#f59e0b"
    } else {
        "#64748b"
    }
}

/// Browser `window.prompt`, hydrate-only (returns `None` on the server build).
#[cfg(feature = "hydrate")]
fn prompt(message: &str) -> Option<String> {
    web_sys::window()?.prompt_with_message(message).ok().flatten()
}

#[cfg(not(feature = "hydrate"))]
fn prompt(_message: &str) -> Option<String> {
    None
}

/// Run a server-fn call, report the outcome, and bump the local refetch (the WS event also
/// arrives — both are idempotent).
fn run(
    status: RwSignal<String>,
    refetch: RwSignal<u32>,
    label: &'static str,
    fut: impl std::future::Future<Output = Result<(), ServerFnError>> + 'static,
) {
    spawn_local(async move {
        match fut.await {
            Ok(()) => {
                status.set(format!("{label}: ok"));
                refetch.update(|n| *n += 1);
            }
            Err(err) => status.set(format!("{label}: {err}")),
        }
    });
}

/// Subscribe to the map's realtime events. Client-only — a no-op on the server build.
#[cfg(feature = "hydrate")]
fn install_wheel_guard(viewport: NodeRef<leptos::html::Div>) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    Effect::new(move |_| {
        if let Some(el) = viewport.get() {
            let cb = Closure::<dyn FnMut(web_sys::WheelEvent)>::new(|ev: web_sys::WheelEvent| {
                ev.prevent_default();
            });
            let opts = web_sys::AddEventListenerOptions::new();
            opts.set_passive(false);
            let _ = el.add_event_listener_with_callback_and_add_event_listener_options(
                "wheel",
                cb.as_ref().unchecked_ref(),
                &opts,
            );
            cb.forget();
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn install_wheel_guard(_viewport: NodeRef<leptos::html::Div>) {}

#[cfg(feature = "hydrate")]
fn start_ws(map_id: i64, refetch: RwSignal<u32>) {
    use futures::StreamExt;
    use gloo_net::websocket::{Message, futures::WebSocket};

    let location = web_sys::window().expect("window").location();
    let scheme = match location.protocol().as_deref() {
        Ok("https:") => "wss",
        _ => "ws",
    };
    let host = location.host().unwrap_or_default();
    let url = format!("{scheme}://{host}/ws/map/{map_id}");

    let mut ws = match WebSocket::open(&url) {
        Ok(ws) => ws,
        Err(_) => return,
    };
    spawn_local(async move {
        while let Some(msg) = ws.next().await {
            match msg {
                Ok(Message::Text(_)) => refetch.update(|n| *n += 1),
                Ok(Message::Bytes(_)) => {}
                Err(_) => break,
            }
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn start_ws(_map_id: i64, _refetch: RwSignal<u32>) {}
