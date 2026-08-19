//! The slash commands.
//!
//! `/vector` with subcommands rather than four top-level names: one entry in the command
//! list is one thing to register, one thing to find, and it keeps Vector's commands from
//! colliding with whatever else lives in the server.
//!
//! Every reply is ephemeral and every command needs a linked account, because everything
//! they answer is about the sender's own maps. The unlinked reply is a link to go and link.

use serde_json::{Value, json};

use crate::auth::AppState;

use super::interactions::{CommandOption, Interaction, focused, option};

/// The command tree, as Discord wants it registered.
pub fn definition() -> Value {
    json!({
        "name": "vector",
        "description": "Your wormhole maps",
        "options": [
            {
                "type": 1,
                "name": "account",
                "description": "Show which Vector account this Discord user is linked to"
            },
            {
                "type": 2,
                "name": "alerts",
                "description": "The alerts you created",
                "options": [
                    {
                        "type": 1, "name": "list", "description": "List the alerts you created",
                        "options": [{
                            "type": 3, "name": "map", "description": "Only this map",
                            "required": false, "autocomplete": true
                        }]
                    },
                    {
                        "type": 1, "name": "enable", "description": "Turn one back on",
                        "options": [{
                            "type": 3, "name": "alert", "description": "Which alert",
                            "required": true, "autocomplete": true
                        }]
                    },
                    {
                        "type": 1, "name": "disable", "description": "Turn one off",
                        "options": [{
                            "type": 3, "name": "alert", "description": "Which alert",
                            "required": true, "autocomplete": true
                        }]
                    },
                    {
                        "type": 1, "name": "remove", "description": "Delete one",
                        "options": [{
                            "type": 3, "name": "alert", "description": "Which alert",
                            "required": true, "autocomplete": true
                        }]
                    }
                ]
            },
            {
                "type": 1,
                "name": "route",
                "description": "How far a system is from one of your chains",
                "options": [
                    {
                        "type": 3, "name": "map", "description": "Which map",
                        "required": true, "autocomplete": true
                    },
                    {
                        "type": 3, "name": "system", "description": "Where to",
                        "required": true, "autocomplete": true
                    }
                ]
            }
        ]
    })
}

/// Upload the command tree to Discord, replacing whatever is registered.
///
/// Global commands, not per-guild: Vector is one application used from many servers, and
/// registering per guild would mean tracking which ones. The cost is that Discord takes a
/// few minutes to roll a change out.
pub async fn register(application_id: &str, bot_token: &str) -> Result<(), String> {
    let response = reqwest::Client::new()
        .put(format!(
            "{}/applications/{application_id}/commands",
            super::API
        ))
        .header("authorization", format!("Bot {bot_token}"))
        .json(&json!([definition()]))
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let body = response.text().await.unwrap_or_default();
    Err(format!("discord returned {status}: {body}"))
}

/// Run a command and produce the reply text.
pub async fn run(state: &AppState, interaction: &Interaction) -> String {
    let Some(sender) = interaction.sender() else {
        return "I could not tell who you are.".into();
    };
    let Some(data) = interaction.data.as_ref() else {
        return "That command arrived empty.".into();
    };
    let user_id = super::user_for(&state.db, &sender.id).await;
    let Some(user_id) = user_id else {
        return unlinked();
    };

    match parse(&data.options) {
        Action::Account => account(state, user_id).await,
        Action::AlertsList { map } => alerts(state, user_id, map).await,
        Action::AlertsSetActive { alert, active } => {
            set_active(state, user_id, alert, active).await
        }
        Action::AlertsRemove { alert } => remove(state, user_id, alert).await,
        Action::Route {
            map: Some(map),
            system: Some(system),
        } => route(state, user_id, map, system).await,
        Action::Route { .. } => "Pick a map and a system from the suggestions.".into(),
        Action::Nothing => "Pick one of the subcommands.".into(),
        Action::Unknown(name) => format!("I do not know `{name}`."),
    }
}

/// What an invocation asks for, once the subcommand tree has been walked.
///
/// Discord nests a group's arguments two levels down, so reading them off the top level
/// finds nothing and says nothing about it. Walking the tree is therefore its own step,
/// with its own tests, rather than a match arm reaching into `options` and hoping.
#[derive(Debug, PartialEq)]
enum Action<'a> {
    Account,
    AlertsList {
        map: Option<&'a str>,
    },
    AlertsSetActive {
        alert: Option<i64>,
        active: bool,
    },
    AlertsRemove {
        alert: Option<i64>,
    },
    Route {
        map: Option<i64>,
        system: Option<i64>,
    },
    /// A name we do not serve: Discord's command list can lag a deploy by minutes.
    Unknown(&'a str),
    Nothing,
}

fn parse(options: &[CommandOption]) -> Action<'_> {
    let Some(sub) = options.first() else {
        return Action::Nothing;
    };
    match sub.name.as_str() {
        "account" => Action::Account,
        "alerts" => {
            let Some(action) = sub.options.first() else {
                return Action::Nothing;
            };
            let alert = option(&action.options, "alert").and_then(|o| o.integer());
            match action.name.as_str() {
                "list" => Action::AlertsList {
                    map: option(&action.options, "map").and_then(|o| o.string()),
                },
                "enable" => Action::AlertsSetActive {
                    alert,
                    active: true,
                },
                "disable" => Action::AlertsSetActive {
                    alert,
                    active: false,
                },
                "remove" => Action::AlertsRemove { alert },
                other => Action::Unknown(other),
            }
        }
        "route" => Action::Route {
            map: option(&sub.options, "map").and_then(|o| o.integer()),
            system: option(&sub.options, "system").and_then(|o| o.integer()),
        },
        other => Action::Unknown(other),
    }
}

fn unlinked() -> String {
    "This Discord account is not linked to a Vector account yet. Open Vector, go to \
     Settings → Discord, and press Connect."
        .into()
}

async fn account(state: &AppState, user_id: i64) -> String {
    let characters = sqlx::query_scalar!(
        "select name from characters where user_id = $1 order by id",
        user_id,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    let maps = sqlx::query_scalar!(
        "select count(distinct m.id) from maps m
         join map_access ma on ma.map_id = m.id
         where ma.subject_id in (
             select id from characters where user_id = $1
             union all select corporation_id from characters where user_id = $1
             union all select alliance_id from characters where user_id = $1 and alliance_id is not null
         )",
        user_id,
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(Some(0))
    .unwrap_or(0);

    if characters.is_empty() {
        return "Linked, but that Vector account has no characters yet.".into();
    }
    format!(
        "Linked to **{}**{}. {maps} {} you can see.",
        characters[0],
        if characters.len() > 1 {
            format!(" and {} other characters", characters.len() - 1)
        } else {
            String::new()
        },
        if maps == 1 { "map" } else { "maps" }
    )
}

async fn alerts(state: &AppState, user_id: i64, map_filter: Option<&str>) -> String {
    let map_id: Option<i64> = map_filter.and_then(|value| value.parse().ok());
    let rows = sqlx::query!(
        "select a.name, a.kind, a.is_active, a.disabled_reason, m.name as map_name
         from map_alerts a
         join maps m on m.id = a.map_id
         where a.created_by_user_id = $1 and ($2::bigint is null or a.map_id = $2)
         order by m.name, a.name",
        user_id,
        map_id,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return "You have not created any alerts.".into();
    }
    let mut out = String::from("**Your alerts**\n");
    for row in rows {
        let state_text = if row.is_active {
            "on".to_string()
        } else {
            format!(
                "off — {}",
                row.disabled_reason.as_deref().unwrap_or("manual")
            )
        };
        out.push_str(&format!(
            "• `{}` {} on **{}** ({state_text})\n",
            row.kind, row.name, row.map_name
        ));
    }
    out
}

/// The alert, if this user created it. Ownership is the permission: an alert you made is
/// yours to turn off from wherever you are, and one you did not is not yours to touch.
async fn owned(state: &AppState, user_id: i64, alert_id: i64) -> Option<(i64, String, i64)> {
    sqlx::query!(
        "select id, name, map_id from map_alerts where id = $1 and created_by_user_id = $2",
        alert_id,
        user_id,
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|row| (row.id, row.name, row.map_id))
}

async fn set_active(state: &AppState, user_id: i64, alert: Option<i64>, active: bool) -> String {
    let Some(alert_id) = alert else {
        return "Pick an alert from the suggestions.".into();
    };
    let Some((id, name, map_id)) = owned(state, user_id, alert_id).await else {
        return "That is not one of your alerts.".into();
    };
    let _ = sqlx::query!(
        "update map_alerts set
             is_active = $2,
             disabled_at = case when $2 then null else now() end,
             disabled_reason = case when $2 then null else 'manual' end,
             updated_at = now()
         where id = $1",
        id,
        active,
    )
    .execute(&state.db)
    .await;
    crate::alerts::log(
        &state.db,
        Some(id),
        map_id,
        Some(user_id),
        if active { "enabled" } else { "disabled" },
        Some("via Discord"),
    )
    .await;
    format!("**{name}** is now {}.", if active { "on" } else { "off" })
}

async fn remove(state: &AppState, user_id: i64, alert: Option<i64>) -> String {
    let Some(alert_id) = alert else {
        return "Pick an alert from the suggestions.".into();
    };
    let Some((id, name, map_id)) = owned(state, user_id, alert_id).await else {
        return "That is not one of your alerts.".into();
    };
    let _ = sqlx::query!("delete from map_alerts where id = $1", id)
        .execute(&state.db)
        .await;
    crate::alerts::log(
        &state.db,
        None,
        map_id,
        Some(user_id),
        "deleted",
        Some(&name),
    )
    .await;
    format!("Deleted **{name}**.")
}

async fn route(state: &AppState, user_id: i64, map_id: i64, system_id: i64) -> String {
    if !can_see(state, user_id, map_id).await {
        return "You do not have access to that map.".into();
    }
    let Some(chain) = crate::alerts::killmail::chain_of(&state.db, map_id).await else {
        return "That map has no systems on it yet.".into();
    };
    let Ok(universe) = crate::alerts::proximity::Universe::load(&state.db).await else {
        return "I could not read the star map just now.".into();
    };
    let Some(found) = crate::alerts::proximity::nearest(
        &universe,
        &chain.systems,
        &chain.edges,
        system_id,
        // Beyond this nobody is flying it anyway, and the search stays bounded.
        30,
    ) else {
        return "That system is more than 30 jumps from the chain.".into();
    };

    let names = sqlx::query!(
        "select id, name from solar_systems where id = any($1)",
        &found.route,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    let named = |id: i64| {
        names
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.name.clone())
            .unwrap_or_else(|| id.to_string())
    };
    let route: Vec<String> = found.route.iter().map(|id| named(*id)).collect();
    format!(
        "**{}** is {} {} from **{}**.\n{}",
        named(system_id),
        found.jumps,
        if found.jumps == 1 { "jump" } else { "jumps" },
        named(found.from),
        route.join(" → ")
    )
}

async fn can_see(state: &AppState, user_id: i64, map_id: i64) -> bool {
    crate::maps::access::effective_role(&state.db, map_id, user_id)
        .await
        .ok()
        .flatten()
        .is_some()
}

/// Suggestions for whichever option is being typed.
pub async fn autocomplete(state: &AppState, interaction: &Interaction) -> Vec<Value> {
    let Some(data) = interaction.data.as_ref() else {
        return Vec::new();
    };
    let Some(field) = focused(&data.options) else {
        return Vec::new();
    };
    let typed = field.string().unwrap_or("").trim().to_lowercase();
    let Some(sender) = interaction.sender() else {
        return Vec::new();
    };
    let Some(user_id) = super::user_for(&state.db, &sender.id).await else {
        return Vec::new();
    };

    match field.name.as_str() {
        "map" => maps_for(state, user_id, &typed).await,
        "system" => systems_like(state, &typed).await,
        "alert" => alerts_for(state, user_id, &typed).await,
        _ => Vec::new(),
    }
}

/// Discord shows at most 25 choices, so ask for that many and no more.
const CHOICES: i64 = 25;

async fn maps_for(state: &AppState, user_id: i64, typed: &str) -> Vec<Value> {
    let like = format!("%{typed}%");
    let rows = sqlx::query!(
        "select distinct m.id, m.name from maps m
         join map_access ma on ma.map_id = m.id
         where m.name ilike $2 and ma.subject_id in (
             select id from characters where user_id = $1
             union all select corporation_id from characters where user_id = $1
             union all select alliance_id from characters where user_id = $1 and alliance_id is not null
         )
         order by m.name
         limit $3",
        user_id,
        like,
        CHOICES,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    rows.into_iter()
        // The value is the id as a string: Discord's autocomplete values are strings, and
        // the command parses them back.
        .map(|row| json!({ "name": row.name, "value": row.id.to_string() }))
        .collect()
}

/// Only the sender's own alerts: they are the only ones these commands can act on.
async fn alerts_for(state: &AppState, user_id: i64, typed: &str) -> Vec<Value> {
    let like = format!("%{typed}%");
    let rows = sqlx::query!(
        "select a.id, a.name, a.is_active, m.name as map_name
         from map_alerts a join maps m on m.id = a.map_id
         where a.created_by_user_id = $1 and a.name ilike $2
         order by m.name, a.name
         limit $3",
        user_id,
        like,
        CHOICES,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    rows.into_iter()
        .map(|row| {
            json!({
                "name": format!(
                    "{} — {}{}",
                    row.name,
                    row.map_name,
                    if row.is_active { "" } else { " (off)" }
                ),
                "value": row.id.to_string()
            })
        })
        .collect()
}

async fn systems_like(state: &AppState, typed: &str) -> Vec<Value> {
    if typed.len() < 2 {
        return Vec::new();
    }
    let contains = format!("%{typed}%");
    let prefix = format!("{typed}%");
    let rows = sqlx::query!(
        "select s.id, s.name, r.name as region
         from solar_systems s join regions r on r.id = s.region_id
         where s.name ilike $1
         order by (s.name ilike $2) desc, length(s.name), s.name
         limit $3",
        contains,
        prefix,
        CHOICES,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    rows.into_iter()
        .map(|row| {
            json!({
                "name": format!("{} — {}", row.name, row.region),
                "value": row.id.to_string()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registered shape is what Discord validates on upload, so it is worth pinning.
    #[test]
    fn the_command_registers_as_one_name_with_subcommands() {
        let definition = definition();
        assert_eq!(definition["name"], "vector");
        let subs = definition["options"].as_array().unwrap();
        assert_eq!(subs.len(), 3);
        for sub in subs {
            // Type 1 is a subcommand, type 2 a group of them.
            assert!(sub["type"] == 1 || sub["type"] == 2);
            assert!(sub["description"].as_str().unwrap().len() > 5);
        }
        // The alerts group is what makes an alert manageable from Discord.
        let alerts = subs.iter().find(|s| s["name"] == "alerts").unwrap();
        let actions: Vec<&str> = alerts["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["name"].as_str().unwrap())
            .collect();
        assert_eq!(actions, vec!["list", "enable", "disable", "remove"]);
    }

    fn options(json: &str) -> Vec<CommandOption> {
        serde_json::from_str::<super::super::interactions::CommandData>(json)
            .unwrap()
            .options
    }

    /// The arguments of a subcommand group sit two levels down. Reading them off the top
    /// level found nothing, so `enable`, `disable` and `remove` all quietly answered with
    /// the list instead of doing anything.
    #[test]
    fn an_alert_action_is_read_from_inside_its_group() {
        let enable = options(
            r#"{"name":"vector","options":[{"name":"alerts","type":2,"options":[
                 {"name":"enable","type":1,"options":[{"name":"alert","value":"42"}]}]}]}"#,
        );
        assert_eq!(
            parse(&enable),
            Action::AlertsSetActive {
                alert: Some(42),
                active: true
            }
        );

        let disable = options(
            r#"{"name":"vector","options":[{"name":"alerts","type":2,"options":[
                 {"name":"disable","type":1,"options":[{"name":"alert","value":"42"}]}]}]}"#,
        );
        assert_eq!(
            parse(&disable),
            Action::AlertsSetActive {
                alert: Some(42),
                active: false
            }
        );

        let remove = options(
            r#"{"name":"vector","options":[{"name":"alerts","type":2,"options":[
                 {"name":"remove","type":1,"options":[{"name":"alert","value":"42"}]}]}]}"#,
        );
        assert_eq!(parse(&remove), Action::AlertsRemove { alert: Some(42) });
    }

    /// The same nesting, and the same bug: `list` filtered by nothing whatever was picked.
    #[test]
    fn listing_alerts_keeps_the_map_it_was_filtered_by() {
        let filtered = options(
            r#"{"name":"vector","options":[{"name":"alerts","type":2,"options":[
                 {"name":"list","type":1,"options":[{"name":"map","value":"7"}]}]}]}"#,
        );
        assert_eq!(parse(&filtered), Action::AlertsList { map: Some("7") });

        let all = options(
            r#"{"name":"vector","options":[{"name":"alerts","type":2,"options":[
                 {"name":"list","type":1}]}]}"#,
        );
        assert_eq!(parse(&all), Action::AlertsList { map: None });
    }

    /// A plain subcommand carries its own arguments, one level down rather than two.
    #[test]
    fn a_plain_subcommand_reads_its_own_arguments() {
        let route = options(
            r#"{"name":"vector","options":[{"name":"route","type":1,"options":[
                 {"name":"map","value":"7"},{"name":"system","value":"30000142"}]}]}"#,
        );
        assert_eq!(
            parse(&route),
            Action::Route {
                map: Some(7),
                system: Some(30000142)
            }
        );
        assert_eq!(
            parse(&options(r#"{"name":"vector","options":[]}"#)),
            Action::Nothing
        );
        assert_eq!(
            parse(&options(
                r#"{"name":"vector","options":[{"name":"wat","type":1}]}"#
            )),
            Action::Unknown("wat")
        );
    }

    #[test]
    fn route_and_alerts_offer_autocomplete_where_a_name_is_wanted() {
        let definition = definition();
        let subs = definition["options"].as_array().unwrap();
        let route = subs.iter().find(|s| s["name"] == "route").unwrap();
        for option in route["options"].as_array().unwrap() {
            assert_eq!(option["autocomplete"], true);
            assert_eq!(option["required"], true);
        }
    }
}
