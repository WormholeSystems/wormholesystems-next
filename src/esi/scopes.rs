/// An ESI scope the application requests at SSO consent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    ReadLocation,
    ReadShipType,
    ReadOnline,
    WriteWaypoint,
}

impl Scope {
    pub const ALL: [Scope; 4] = [
        Scope::ReadLocation,
        Scope::ReadShipType,
        Scope::ReadOnline,
        Scope::WriteWaypoint,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Scope::ReadLocation => "esi-location.read_location.v1",
            Scope::ReadShipType => "esi-location.read_ship_type.v1",
            Scope::ReadOnline => "esi-location.read_online.v1",
            Scope::WriteWaypoint => "esi-ui.write_waypoint.v1",
        }
    }

    pub fn parse(s: &str) -> Option<Scope> {
        Scope::ALL.into_iter().find(|scope| scope.as_str() == s)
    }
}
