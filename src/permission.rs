use serde::{Serialize, Deserialize};
use crate::u53::u53;

/// A unique number assigned to new Permissions by the Authority.
pub type Nonce = u53;

/// Actions are identified by a string such as "sign-in" or "send-message".
pub type Action = String;

/// A Permission is a versioned collection of actions.
#[derive(Serialize, Deserialize, Clone)]
pub struct Permission {

    /// The Permission's unique nonce.
    ///
    /// This acts like an ID and will persist for the Permission across
    /// its lifetime of updates.
    pub nonce: Nonce,

    /// The actions this Permission allows its owner to take.
    pub actions: Vec<Action>,

    /// The version of the Permission.
    ///
    /// This must be incremented every time the Permission is updated with
    /// different actions.
    pub version: usize,
}
