//! The identity every outbound request carries.
//!
//! CCP, zKillboard and EVE Ref all ask that a client says who is running it and how to
//! reach them: an anonymous client gets rate limited, and a misbehaving one that nobody can
//! contact gets blocked. That contact has to be whoever deployed this copy, not whoever
//! wrote it, so it comes from the environment and the process refuses to run without it.

use std::sync::OnceLock;

pub const NAME_VAR: &str = "VECTOR_CONTACT_NAME";
pub const EMAIL_VAR: &str = "VECTOR_CONTACT_EMAIL";

static AGENT: OnceLock<String> = OnceLock::new();

/// `vector/0.1.0 (Some Pilot; someone@example.com; +https://github.com/eve-vector/vector)`
///
/// Panics if the contact is not configured. That is deliberate: a deploy that talks to ESI
/// anonymously is one that gets throttled and then banned, and failing at the first request
/// with a message naming the two variables is kinder than finding out from CCP.
pub fn get() -> &'static str {
    AGENT.get_or_init(|| {
        let name = contact(NAME_VAR);
        let email = contact(EMAIL_VAR);
        format!(
            "vector/{} ({name}; {email}; +https://github.com/eve-vector/vector)",
            env!("CARGO_PKG_VERSION")
        )
    })
}

fn contact(var: &str) -> String {
    match std::env::var(var) {
        Ok(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => panic!(
            "{NAME_VAR} and {EMAIL_VAR} must be set: every request to ESI, zKillboard and \
             EVE Ref carries them so those services can tell who is running this and reach \
             them. Use the in-game name of the character who administers this install and \
             an address you read."
        ),
    }
}

/// A client that identifies itself. Every outbound client in the app is built from one of
/// these two, so there is one place the identity can be wrong.
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(get())
        .build()
        .expect("http client")
}

pub fn blocking_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent(get())
        .build()
        .expect("http client")
}

#[cfg(test)]
mod tests {
    /// The format is what the services read, so it is worth pinning: product/version first,
    /// then who to contact.
    #[test]
    fn names_the_product_and_the_contact() {
        // SAFETY: single-threaded test process, and nothing else reads these here.
        unsafe {
            std::env::set_var(super::NAME_VAR, "  Zvi Sarok  ");
            std::env::set_var(super::EMAIL_VAR, "admin@example.com");
        }
        let agent = super::get();
        assert!(agent.starts_with("vector/"), "{agent}");
        assert!(agent.contains("(Zvi Sarok; admin@example.com;"), "{agent}");
        assert!(
            agent.contains("+https://github.com/eve-vector/vector"),
            "{agent}"
        );
    }
}
