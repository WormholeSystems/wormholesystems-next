/// The single error type returned by every map action. See
/// [`docs/features/maps.md`](../../docs/features/maps.md#error-model) for the mapping
/// of variant → situation that the action contracts (and their tests) rely on.
#[derive(Debug, thiserror::Error)]
pub enum MapError {
    /// The map (or a referenced row) doesn't exist, or the user has no access to it —
    /// the two are deliberately indistinguishable, so we don't leak a map's existence.
    #[error("not found")]
    NotFound,

    /// The user can see the map but holds a lower role than the action requires (or is
    /// acting as a character that isn't theirs).
    #[error("forbidden")]
    Forbidden,

    /// A uniqueness / idempotency violation: a system already placed, a connection that
    /// already exists.
    #[error("conflict: {0}")]
    Conflict(String),

    /// Bad input: a blank name, a self-connection, an endpoint not on the map.
    #[error("invalid: {0}")]
    Validation(String),

    /// The operation would leave the map with zero owners; every map keeps at least one.
    #[error("a map must always have at least one owner")]
    LastOwner,

    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, MapError>;
