//! What the wizard collects, and what that turns into on disk.
//!
//! Separate from the prompting so the mapping can be tested without a terminal.

use std::collections::BTreeMap;

use rand::Rng;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answers {
    /// Empty means no domain: plain http on this machine.
    pub domain: String,
    pub http_port: u16,
    pub https_port: u16,
    pub contact_name: String,
    pub contact_email: String,
    pub eve_client_id: String,
    pub eve_client_secret: String,
    pub discord: Option<Discord>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discord {
    pub application_id: String,
    pub public_key: String,
    pub client_id: String,
    pub client_secret: String,
    /// Only needed to post as the bot or send direct messages.
    pub bot_token: String,
}

impl Answers {
    /// What the outside world calls this install. Every redirect has to match it exactly.
    pub fn base_url(&self) -> String {
        if self.domain.is_empty() {
            format!("http://localhost:{}", self.http_port)
        } else {
            format!("https://{}", self.domain)
        }
    }

    /// The `.env` keys this produces. Discord is absent rather than blank when it is off:
    /// the server reads it all-or-nothing, and an empty client id counts as configured.
    pub fn env_values(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        let mut set = |k: &str, v: String| {
            values.insert(k.to_string(), v);
        };
        set("WS_DOMAIN", self.domain.clone());
        set("HTTP_PORT", self.http_port.to_string());
        set("HTTPS_PORT", self.https_port.to_string());
        set("WS_CONTACT_NAME", self.contact_name.clone());
        set("WS_CONTACT_EMAIL", self.contact_email.clone());
        set("EVE_CLIENT_ID", self.eve_client_id.clone());
        set("EVE_CLIENT_SECRET", self.eve_client_secret.clone());
        set(
            "EVE_REDIRECT_URI",
            format!("{}/auth/callback", self.base_url()),
        );
        if let Some(discord) = &self.discord {
            set("DISCORD_APPLICATION_ID", discord.application_id.clone());
            set("DISCORD_PUBLIC_KEY", discord.public_key.clone());
            set("DISCORD_CLIENT_ID", discord.client_id.clone());
            set("DISCORD_CLIENT_SECRET", discord.client_secret.clone());
            set(
                "DISCORD_REDIRECT_URI",
                format!("{}/discord/callback", self.base_url()),
            );
            if !discord.bot_token.is_empty() {
                set("DISCORD_BOT_TOKEN", discord.bot_token.clone());
            }
        }
        values
    }

    /// The keys to strike out, for a setup that turned Discord off after having it on.
    pub fn env_removals(&self) -> Vec<&'static str> {
        const DISCORD: [&str; 6] = [
            "DISCORD_APPLICATION_ID",
            "DISCORD_PUBLIC_KEY",
            "DISCORD_CLIENT_ID",
            "DISCORD_CLIENT_SECRET",
            "DISCORD_REDIRECT_URI",
            "DISCORD_BOT_TOKEN",
        ];
        match &self.discord {
            None => DISCORD.to_vec(),
            Some(d) if d.bot_token.is_empty() => vec!["DISCORD_BOT_TOKEN"],
            Some(_) => Vec::new(),
        }
    }
}

/// A password nobody has to think about. Alphanumeric so it survives a URL, a shell and a
/// compose file without escaping.
pub fn generated_password() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..32)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answers() -> Answers {
        Answers {
            domain: "map.example.com".into(),
            http_port: 80,
            https_port: 443,
            contact_name: "Nicolas Kion".into(),
            contact_email: "me@example.com".into(),
            eve_client_id: "abc".into(),
            eve_client_secret: "shh".into(),
            discord: None,
        }
    }

    #[test]
    fn a_domain_means_https_and_no_domain_means_the_local_port() {
        assert_eq!(answers().base_url(), "https://map.example.com");
        let local = Answers {
            domain: String::new(),
            http_port: 8080,
            ..answers()
        };
        assert_eq!(local.base_url(), "http://localhost:8080");
    }

    // The EVE application rejects a callback that is not character-for-character the one
    // registered, so this is derived rather than typed a second time.
    #[test]
    fn the_callback_is_derived_from_the_base_url() {
        let values = answers().env_values();
        assert_eq!(
            values["EVE_REDIRECT_URI"],
            "https://map.example.com/auth/callback"
        );
    }

    #[test]
    fn discord_off_writes_nothing_and_strikes_the_keys_out() {
        let values = answers().env_values();
        assert!(!values.keys().any(|k| k.starts_with("DISCORD_")));
        assert!(answers().env_removals().contains(&"DISCORD_CLIENT_ID"));
    }

    #[test]
    fn discord_on_derives_its_own_callback() {
        let with = Answers {
            discord: Some(Discord {
                application_id: "1".into(),
                public_key: "k".into(),
                client_id: "2".into(),
                client_secret: "s".into(),
                bot_token: String::new(),
            }),
            ..answers()
        };
        let values = with.env_values();
        assert_eq!(
            values["DISCORD_REDIRECT_URI"],
            "https://map.example.com/discord/callback"
        );
        // No bot token given, so the key is removed rather than written empty.
        assert!(!values.contains_key("DISCORD_BOT_TOKEN"));
        assert_eq!(with.env_removals(), vec!["DISCORD_BOT_TOKEN"]);
    }

    #[test]
    fn the_generated_password_needs_no_escaping() {
        let password = generated_password();
        assert_eq!(password.len(), 32);
        assert!(password.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
