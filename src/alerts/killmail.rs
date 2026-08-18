//! "Tell me when something dies near us."
//!
//! Every killmail the ingest stores is offered to every active killmail alert. An alert
//! fires when its filters match the kill *and* the kill happened within its jump range of
//! the map, so a corp-wide watch on an alliance only pings the chains that are actually
//! close to it.
//!
//! Runs after the killmail is written and off the ingest's critical path: the stream is
//! sequential, and a slow Discord must not stall it.

use sqlx::PgPool;

use super::delivery::{Embed, Field, Footer, Image, Message, post_webhook, security_color};
use super::proximity::{self, Universe};
use super::{Alert, AlertDelivery, AlertKind, AlertMention, DisabledReason, filters};

/// Everything a message needs to say about one kill.
pub struct Kill {
    pub id: i64,
    pub solar_system_id: i64,
    pub candidates: filters::Candidates,
    pub victim_name: Option<String>,
    pub victim_ship: Option<String>,
    pub victim_ship_type_id: Option<i64>,
    pub attacker_name: Option<String>,
    pub total_value: Option<f64>,
    pub attacker_count: Option<i32>,
    pub is_solo: bool,
    pub is_npc: bool,
}

/// Offer a killmail to every alert watching for one.
pub async fn evaluate(pool: &PgPool, http: &reqwest::Client, universe: &Universe, kill: &Kill) {
    let Ok(alerts) = super::active(pool, AlertKind::Killmail).await else {
        return;
    };
    if alerts.is_empty() {
        return;
    }
    // Filters are cheap and the route is not, so rule the alert out on its filters first.
    let interested: Vec<&Alert> = alerts
        .iter()
        .filter(|a| filters::matches(&a.filters, a.filter_match, &kill.candidates))
        .collect();
    if interested.is_empty() {
        return;
    }

    let Ok(system) = system_name(pool, kill.solar_system_id).await else {
        return;
    };
    for alert in interested {
        let Some(chain) = chain_of(pool, alert.map_id).await else {
            continue;
        };
        let Some(found) = proximity::nearest(
            universe,
            &chain.systems,
            &chain.edges,
            kill.solar_system_id,
            alert.max_jumps,
        ) else {
            continue;
        };
        // The claim is per killmail, so the same kill can never ping the same alert twice
        // however many times the stream replays it.
        let key = format!("killmail:{}", kill.id);
        if !super::claim(pool, alert.id, &key).await {
            continue;
        }
        let from = system_name(pool, found.from)
            .await
            .map(|s| s.name)
            .unwrap_or_default();
        let embed = build(kill, &system, &from, found.jumps);
        match send(pool, http, alert, embed).await {
            Ok(()) => super::sent(pool, alert.id, &key).await,
            Err(fatal) => {
                super::unclaim(pool, alert.id, &key).await;
                if fatal {
                    super::disable(pool, alert, DisabledReason::DestinationGone, None).await;
                }
            }
        }
    }
}

/// Send one message, returning whether the failure was terminal.
async fn send(
    pool: &PgPool,
    http: &reqwest::Client,
    alert: &Alert,
    embed: Embed,
) -> Result<(), bool> {
    let message = match alert.mention {
        AlertMention::Role => match alert.discord_role_id.as_deref() {
            Some(role) => Message::new(embed).mention_role(role),
            None => Message::new(embed),
        },
        AlertMention::Everyone => Message::new(embed).mention_everyone(),
        // Pinging the creator needs their Discord id, which arrives with account linking.
        AlertMention::Creator | AlertMention::None => Message::new(embed),
    };

    match alert.delivery {
        AlertDelivery::Webhook => {
            let Some(url) = alert.webhook_url.as_deref() else {
                return Err(true);
            };
            match post_webhook(http, url, &message).await {
                Ok(()) => Ok(()),
                Err(super::delivery::SendError::Gone) => Err(true),
                Err(super::delivery::SendError::Failed(err)) => {
                    super::log(
                        pool,
                        Some(alert.id),
                        alert.map_id,
                        None,
                        "failed",
                        Some(&err),
                    )
                    .await;
                    Err(false)
                }
            }
        }
        // Bot deliveries are configured but not yet dispatchable; leaving the claim
        // released means they start working the moment the bot lands.
        AlertDelivery::DiscordDm | AlertDelivery::DiscordChannel => Err(false),
    }
}

fn build(kill: &Kill, system: &System, from: &str, jumps: i32) -> Embed {
    let victim = kill.victim_name.as_deref().unwrap_or("Someone");
    let ship = kill.victim_ship.as_deref().unwrap_or("a ship");
    let where_ = if jumps == 0 {
        format!("in {}", system.name)
    } else if jumps == 1 {
        format!("in {}, 1 jump from {from}", system.name)
    } else {
        format!("in {}, {jumps} jumps from {from}", system.name)
    };

    let mut fields = vec![
        Field::new(
            "System",
            format!("{} ({:.1})", system.name, system.security),
            true,
        ),
        Field::new("Jumps", jumps.to_string(), true),
    ];
    if let Some(value) = kill.total_value {
        fields.push(Field::new("Value", isk(value), true));
    }
    if let Some(name) = kill.attacker_name.as_deref() {
        fields.push(Field::new("Final blow", name, true));
    }
    if let Some(count) = kill.attacker_count {
        let label = if kill.is_solo {
            "Solo".to_string()
        } else {
            count.to_string()
        };
        fields.push(Field::new("Attackers", label, true));
    }

    Embed {
        title: format!("{victim} lost {ship}"),
        url: Some(format!("https://zkillboard.com/kill/{}/", kill.id)),
        description: Some(where_),
        color: security_color(system.security),
        fields,
        thumbnail: kill.victim_ship_type_id.map(|id| Image {
            url: format!("https://images.evetech.net/types/{id}/render?size=128"),
        }),
        footer: Some(Footer {
            text: if kill.is_npc {
                "NPC kill".into()
            } else {
                "zKillboard".into()
            },
        }),
        timestamp: None,
    }
}

/// Round ISK the way a reader thinks about it: billions matter, the decimals do not.
fn isk(value: f64) -> String {
    if value >= 1_000_000_000.0 {
        format!("{:.1}B", value / 1_000_000_000.0)
    } else if value >= 1_000_000.0 {
        format!("{:.0}M", value / 1_000_000.0)
    } else {
        format!("{:.0}K", value / 1_000.0)
    }
}

pub struct System {
    pub name: String,
    pub security: f64,
}

async fn system_name(pool: &PgPool, id: i64) -> sqlx::Result<System> {
    sqlx::query!(
        "select name, security_status from solar_systems where id = $1",
        id,
    )
    .fetch_one(pool)
    .await
    .map(|row| System {
        name: row.name,
        security: row.security_status,
    })
}

pub struct Chain {
    pub systems: Vec<i64>,
    pub edges: Vec<(i64, i64)>,
}

/// A map's placed systems and the connections between them, in solar-system ids.
pub async fn chain_of(pool: &PgPool, map_id: i64) -> Option<Chain> {
    let systems: Vec<i64> = sqlx::query_scalar!(
        "select solar_system_id from map_solar_systems where map_id = $1",
        map_id,
    )
    .fetch_all(pool)
    .await
    .ok()?;
    if systems.is_empty() {
        return None;
    }
    let edges = sqlx::query!(
        "select a.solar_system_id as from_id, b.solar_system_id as to_id
         from map_connections c
         join map_solar_systems a on a.id = c.from_system
         join map_solar_systems b on b.id = c.to_system
         where c.map_id = $1",
        map_id,
    )
    .fetch_all(pool)
    .await
    .ok()?
    .into_iter()
    .map(|row| (row.from_id, row.to_id))
    .collect();
    Some(Chain { systems, edges })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isk_reads_at_the_scale_that_matters() {
        assert_eq!(isk(2_400_000_000.0), "2.4B");
        assert_eq!(isk(45_000_000.0), "45M");
        assert_eq!(isk(9_000.0), "9K");
    }

    #[test]
    fn the_title_names_the_loss_and_the_body_places_it() {
        let kill = Kill {
            id: 1,
            solar_system_id: 30000142,
            candidates: filters::Candidates::default(),
            victim_name: Some("Pilot".into()),
            victim_ship: Some("a Loki".into()),
            victim_ship_type_id: Some(29990),
            attacker_name: None,
            total_value: Some(2_400_000_000.0),
            attacker_count: Some(4),
            is_solo: false,
            is_npc: false,
        };
        let system = System {
            name: "Jita".into(),
            security: 0.9,
        };
        let embed = build(&kill, &system, "Perimeter", 2);
        assert_eq!(embed.title, "Pilot lost a Loki");
        assert_eq!(
            embed.description.as_deref(),
            Some("in Jita, 2 jumps from Perimeter")
        );
        assert!(embed.fields.iter().any(|f| f.value == "2.4B"));
    }

    #[test]
    fn a_kill_in_the_chain_says_so_without_a_jump_count() {
        let kill = Kill {
            id: 2,
            solar_system_id: 31001882,
            candidates: filters::Candidates::default(),
            victim_name: None,
            victim_ship: None,
            victim_ship_type_id: None,
            attacker_name: None,
            total_value: None,
            attacker_count: None,
            is_solo: false,
            is_npc: false,
        };
        let system = System {
            name: "J122515".into(),
            security: -0.99,
        };
        let embed = build(&kill, &system, "J122515", 0);
        assert_eq!(embed.title, "Someone lost a ship");
        assert_eq!(embed.description.as_deref(), Some("in J122515"));
    }
}
