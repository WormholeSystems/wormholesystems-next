use super::{EsiClient, EsiError, Result};

impl EsiClient {
    /// `destination_id` is a solar system, station, or structure id.
    pub async fn set_waypoint(
        &self,
        token: &str,
        destination_id: i64,
        add_to_beginning: bool,
        clear_other_waypoints: bool,
    ) -> Result<()> {
        // Values are plain ints/bools, so no escaping is needed.
        let path = format!(
            "/ui/autopilot/waypoint?destination_id={destination_id}\
             &add_to_beginning={add_to_beginning}&clear_other_waypoints={clear_other_waypoints}"
        );
        let resp = self
            .request(reqwest::Method::POST, &path, Some(token))
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(EsiError::Api {
                status: status.as_u16(),
                body,
            });
        }
        Ok(())
    }
}
