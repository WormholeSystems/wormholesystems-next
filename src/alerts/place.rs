//! "Tell me when the chain gets near somewhere."
//!
//! Fires when a system is placed on the map: the chain just changed shape, so anything it
//! was too far from a moment ago might not be now. Legacy's framing, and the right one —
//! a proximity alert is about the map moving, not about the target.
//!
//! The claim is per placement, so a system added, removed and added again is a new
//! occasion worth a message, while a hundred other edits are not.

use sqlx::PgPool;

use super::delivery::{Embed, Field, security_color};
use super::killmail::chain_of;
use super::proximity::{self, Universe};
use super::{Alert, AlertKind, DisabledReason};

/// Evaluate every proximity alert on a map after a system landed on it.
pub async fn evaluate(
    pool: &PgPool,
    http: &reqwest::Client,
    bot_token: Option<&str>,
    universe: &Universe,
    map_id: i64,
    map_solar_system_id: i64,
) {
    let Ok(alerts) = super::active(pool, AlertKind::Proximity).await else {
        return;
    };
    let mine: Vec<&Alert> = alerts.iter().filter(|a| a.map_id == map_id).collect();
    if mine.is_empty() {
        return;
    }
    let Some(chain) = chain_of(pool, map_id).await else {
        return;
    };
    // What was just added, so the message can say what changed rather than only what is
    // now reachable.
    let added = sqlx::query!(
        "select ss.id, ss.name from map_solar_systems mss
         join solar_systems ss on ss.id = mss.solar_system_id
         where mss.id = $1",
        map_solar_system_id,
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    for alert in mine {
        let Some(target_id) = alert.target_solar_system_id else {
            continue;
        };
        let Some(found) = proximity::nearest(
            universe,
            &chain.systems,
            &chain.edges,
            target_id,
            alert.max_jumps,
        ) else {
            continue;
        };
        let key = format!("placement:{map_solar_system_id}");
        if !super::claim(pool, alert.id, &key).await {
            continue;
        }
        let Ok(names) = names(pool, &found.route).await else {
            super::unclaim(pool, alert.id, &key).await;
            continue;
        };
        let embed = build(
            &names,
            &found,
            added.as_ref().map(|row| row.name.as_str()),
            map_id,
        );
        match super::deliver(pool, http, bot_token, alert, embed).await {
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

/// One system's display data, in route order.
pub struct Named {
    pub id: i64,
    pub name: String,
    pub security: f64,
}

async fn names(pool: &PgPool, route: &[i64]) -> sqlx::Result<Vec<Named>> {
    let rows = sqlx::query!(
        "select id, name, security_status from solar_systems where id = any($1)",
        route,
    )
    .fetch_all(pool)
    .await?;
    // Back into route order: the query returns them however the index felt like.
    Ok(route
        .iter()
        .filter_map(|id| {
            rows.iter().find(|r| r.id == *id).map(|r| Named {
                id: r.id,
                name: r.name.clone(),
                security: r.security_status,
            })
        })
        .collect())
}

fn build(route: &[Named], found: &proximity::Proximity, added: Option<&str>, map_id: i64) -> Embed {
    let target = route.last();
    let target_name = target.map(|s| s.name.as_str()).unwrap_or("Somewhere");
    let from = route
        .first()
        .map(|s| s.name.as_str())
        .unwrap_or("the chain");
    let jumps = found.jumps;

    let description = match added {
        Some(added) if jumps == 0 => format!("**{added}** is **{target_name}**."),
        Some(added) => format!(
            "**{added}** was just added, putting **{target_name}** {jumps} {} away.",
            if jumps == 1 { "jump" } else { "jumps" }
        ),
        None => format!("The chain now reaches **{target_name}**."),
    };

    let mut fields = vec![
        Field::new(
            "Target",
            target
                .map(|s| format!("{} ({:.1})", s.name, s.security))
                .unwrap_or_else(|| target_name.to_string()),
            true,
        ),
        Field::new("Gate jumps", jumps.to_string(), true),
        Field::new("From", from, true),
    ];
    if route.len() > 1 {
        let names: Vec<&str> = route.iter().map(|s| s.name.as_str()).collect();
        fields.push(Field::new("Route", names.join(" → "), false));
    }

    Embed {
        title: format!(
            "{target_name} is {jumps} {} out",
            if jumps == 1 { "jump" } else { "jumps" }
        ),
        url: Some(format!("/maps/{map_id}")),
        description: Some(description),
        color: security_color(target.map(|s| s.security).unwrap_or(-1.0)),
        fields,
        thumbnail: None,
        footer: None,
        timestamp: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(id: i64, name: &str, security: f64) -> Named {
        Named {
            id,
            name: name.into(),
            security,
        }
    }

    #[test]
    fn the_message_names_what_changed_and_the_way_there() {
        let route = vec![
            named(1, "J122515", -0.99),
            named(2, "Amarr", 1.0),
            named(3, "Ashab", 1.0),
        ];
        let found = proximity::Proximity {
            jumps: 2,
            from: 1,
            route: vec![1, 2, 3],
        };
        let embed = build(&route, &found, Some("J122515"), 7);
        assert_eq!(embed.title, "Ashab is 2 jumps out");
        assert!(embed.description.as_deref().unwrap().contains("J122515"));
        assert!(
            embed
                .fields
                .iter()
                .any(|f| f.value == "J122515 → Amarr → Ashab")
        );
    }

    #[test]
    fn one_jump_reads_as_one_jump() {
        let route = vec![named(1, "Perimeter", 1.0), named(2, "Jita", 0.9)];
        let found = proximity::Proximity {
            jumps: 1,
            from: 1,
            route: vec![1, 2],
        };
        let embed = build(&route, &found, Some("Perimeter"), 1);
        assert_eq!(embed.title, "Jita is 1 jump out");
        assert!(
            embed
                .description
                .as_deref()
                .unwrap()
                .contains("1 jump away")
        );
    }

    /// Placing the target itself is worth saying plainly rather than "0 jumps away".
    #[test]
    fn placing_the_target_says_so() {
        let route = vec![named(1, "Jita", 0.9)];
        let found = proximity::Proximity {
            jumps: 0,
            from: 1,
            route: vec![1],
        };
        let embed = build(&route, &found, Some("Jita"), 1);
        assert_eq!(embed.description.as_deref(), Some("**Jita** is **Jita**."));
    }
}
