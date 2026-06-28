//! A single map: the live graph (SVG + signatures), refetched whenever a
//! [`MapEvent`](crate::maps::MapEvent) arrives over the WebSocket, plus a minimal editing
//! toolbar. The acting character comes from the session — the server functions take no
//! actor. Open the same map in two browsers to watch edits propagate in realtime.

use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::app::api::{
    add_connection, add_signature, add_system, fetch_map, link_signature, list_signatures,
    set_connection_status,
};
use crate::maps::connection::{AddConnection, SetConnectionStatus};
use crate::maps::signatures::AddSignature;
use crate::maps::solar_system::AddSystem;
use crate::maps::{ConnectionType, MapView, MassStatus, Signature, SignatureGroup, TimeStatus};

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

    let map = RwSignal::new(None::<MapView>);
    let sigs = RwSignal::new(None::<Vec<Signature>>);
    let log = RwSignal::new(Vec::<String>::new());
    let status = RwSignal::new(String::new());
    let system_input = RwSignal::new("30000142".to_string());
    let refetch = RwSignal::new(0u32);

    // Load (and reload on events) the map + signatures for the routed id.
    Effect::new(move |_| {
        refetch.track();
        let id = map_id();
        if id == 0 {
            return;
        }
        spawn_local(async move {
            match fetch_map(id).await {
                Ok(mv) => map.set(Some(mv)),
                Err(err) => status.set(err.to_string()),
            }
            if let Ok(s) = list_signatures(id).await {
                sigs.set(Some(s));
            }
        });
    });

    // Connect the realtime stream once the id is known.
    Effect::new(move |prev: Option<i64>| {
        let id = map_id();
        if id != 0 && prev != Some(id) {
            start_ws(id, log, refetch);
        }
        id
    });

    let add = move |_| {
        let id = map_id();
        let n = map.get_untracked().map(|m| m.systems.len()).unwrap_or(0);
        let solar_system_id = system_input
            .get_untracked()
            .trim()
            .parse::<i64>()
            .unwrap_or(30000142);
        let cmd = AddSystem {
            map_id: id,
            solar_system_id,
            x: 90.0 + (n % 4) as f64 * 130.0,
            y: 110.0 + (n / 4) as f64 * 120.0,
            alias: None,
        };
        run(status, refetch, "add system", async move {
            add_system(cmd).await.map(|_| ())
        });
    };

    let connect = move |_| {
        let Some(mv) = map.get_untracked() else {
            return;
        };
        if mv.systems.len() < 2 {
            status.set("connect: need two systems".into());
            return;
        }
        let cmd = AddConnection {
            map_id: map_id(),
            from_system: mv.systems[0].id,
            to_system: mv.systems[1].id,
            kind: ConnectionType::Wormhole,
        };
        run(status, refetch, "connect first two", async move {
            add_connection(cmd).await.map(|_| ())
        });
    };

    let add_sig = move |_| {
        let Some(mv) = map.get_untracked() else {
            return;
        };
        let Some(s) = mv.systems.first() else {
            status.set("add sig: place a system first".into());
            return;
        };
        let cmd = AddSignature {
            map_id: map_id(),
            solar_system_id: s.solar_system_id,
            signature_id: "ABC-123".into(),
            group: SignatureGroup::Wormhole,
            mass_status: Some(MassStatus::Stable),
            time_status: Some(TimeStatus::Eol),
            ..Default::default()
        };
        run(status, refetch, "add wormhole sig", async move {
            add_signature(cmd).await.map(|_| ())
        });
    };

    let link = move |_| {
        let (Some(mv), Some(ss)) = (map.get_untracked(), sigs.get_untracked()) else {
            return;
        };
        let (Some(c), Some(sig)) = (mv.connections.first(), ss.first()) else {
            status.set("link: need a connection and a signature".into());
            return;
        };
        let cmd = crate::maps::signatures::LinkSignature {
            map_id: map_id(),
            signature_pk: sig.id,
            connection_id: c.id,
        };
        run(status, refetch, "link sig -> conn", async move {
            link_signature(cmd).await.map(|_| ())
        });
    };

    let mark = move |mass: MassStatus, time: TimeStatus, label: &'static str| {
        move |_| {
            let Some(mv) = map.get_untracked() else {
                return;
            };
            let Some(c) = mv.connections.first() else {
                status.set("no connection to mark".into());
                return;
            };
            let cmd = SetConnectionStatus {
                map_id: map_id(),
                connection_id: c.id,
                mass_status: Some(Some(mass)),
                time_status: Some(Some(time)),
                size: None,
            };
            run(status, refetch, label, async move {
                set_connection_status(cmd).await.map(|_| ())
            });
        }
    };

    let btn = "px-2.5 py-1 rounded bg-slate-700 text-slate-100 text-sm hover:bg-slate-600";

    view! {
        <div class="flex items-center justify-between">
            <a href="/maps" class="text-sm text-blue-600 hover:underline">"← Maps"</a>
            <span class="text-sm text-slate-500">{move || status.get()}</span>
        </div>

        <div class="mt-3 flex flex-wrap items-center gap-2">
            <input
                class="border rounded px-2 py-1 text-sm w-32"
                placeholder="SDE system id"
                prop:value=move || system_input.get()
                on:input=move |ev| system_input.set(event_target_value(&ev))
            />
            <button class=btn on:click=add>"Add system"</button>
            <button class=btn on:click=connect>"Connect first two"</button>
            <button class=btn on:click=add_sig>"Add wh sig"</button>
            <button class=btn on:click=link>"Link sig"</button>
            <button class=btn on:click=mark(MassStatus::Critical, TimeStatus::Critical, "mark critical")>
                "Mark critical"
            </button>
            <button class=btn on:click=mark(MassStatus::Stable, TimeStatus::Stable, "mark stable")>
                "Reset stable"
            </button>
        </div>

        <div class="mt-4 grid grid-cols-3 gap-4">
            <div class="col-span-2">{move || map_svg(map.get())}</div>
            <div class="space-y-4">
                <div>
                    <h2 class="font-semibold text-sm">"Signatures"</h2>
                    <ul class="mt-1 text-xs font-mono space-y-0.5">
                        {move || {
                            sigs.get()
                                .unwrap_or_default()
                                .into_iter()
                                .map(|s| view! { <li>{sig_line(&s)}</li> })
                                .collect_view()
                        }}
                    </ul>
                </div>
                <div>
                    <h2 class="font-semibold text-sm">"Events"</h2>
                    <ul class="mt-1 text-xs font-mono space-y-0.5 max-h-64 overflow-auto">
                        {move || {
                            log.get()
                                .into_iter()
                                .rev()
                                .take(25)
                                .map(|m| view! { <li>{m}</li> })
                                .collect_view()
                        }}
                    </ul>
                </div>
            </div>
        </div>
    }
}

/// Run a server-fn call, report the outcome, and bump the local refetch (the WS event will
/// also arrive — both are idempotent).
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

fn sig_line(s: &Signature) -> String {
    let link = s
        .connection_id
        .map(|c| format!(" -> conn#{c}"))
        .unwrap_or_default();
    format!(
        "{} [{}] m={:?} t={:?}{link}",
        s.signature_id,
        s.group.as_str(),
        s.mass_status,
        s.time_status
    )
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

fn map_svg(map: Option<MapView>) -> impl IntoView {
    let Some(mv) = map else {
        return view! {
            <div class="h-[420px] grid place-items-center text-slate-500 bg-slate-900 rounded">
                "Loading…"
            </div>
        }
        .into_any();
    };

    let pos: HashMap<i64, (f64, f64)> = mv
        .systems
        .iter()
        .map(|s| (s.id, (s.position_x, s.position_y)))
        .collect();

    let edges = mv
        .connections
        .iter()
        .filter_map(|c| {
            let (x1, y1) = pos.get(&c.from_system).copied()?;
            let (x2, y2) = pos.get(&c.to_system).copied()?;
            let color = edge_color(c.mass_status, c.time_status);
            Some(view! { <line x1=x1 y1=y1 x2=x2 y2=y2 stroke=color stroke-width="3" /> })
        })
        .collect_view();

    let nodes = mv
        .systems
        .iter()
        .map(|s| {
            let (cx, cy) = (s.position_x, s.position_y);
            let label = s
                .alias
                .clone()
                .unwrap_or_else(|| s.solar_system_id.to_string());
            view! {
                <circle cx=cx cy=cy r="24" fill="#1e293b" stroke="#475569" stroke-width="2" />
                <text x=cx y=cy text-anchor="middle" dy="4" fill="#e2e8f0" font-size="11">
                    {label}
                </text>
            }
        })
        .collect_view();

    view! {
        <svg viewBox="0 0 600 450" class="w-full h-[420px] bg-slate-900 rounded">
            {edges}
            {nodes}
        </svg>
    }
    .into_any()
}

/// Subscribe to the map's realtime events. Client-only — a no-op on the server build.
#[cfg(feature = "hydrate")]
fn start_ws(map_id: i64, log: RwSignal<Vec<String>>, refetch: RwSignal<u32>) {
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
        Err(err) => {
            log.update(|l| l.push(format!("ws open failed: {err}")));
            return;
        }
    };
    spawn_local(async move {
        while let Some(msg) = ws.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    log.update(|l| l.push(text));
                    refetch.update(|n| *n += 1);
                }
                Ok(Message::Bytes(_)) => {}
                Err(err) => {
                    log.update(|l| l.push(format!("ws error: {err}")));
                    break;
                }
            }
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn start_ws(_map_id: i64, _log: RwSignal<Vec<String>>, _refetch: RwSignal<u32>) {}
