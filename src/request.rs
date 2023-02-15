use clacc::Witness;
use gmp::mpz::Mpz;
use serde::{Serialize, Deserialize};
use crate::permission::{Action, Permission};

/// A request to perform an action.
#[derive(Deserialize, Serialize)]
pub struct ActionRequest {

    /// The Permission associated with the action.
    pub perm: Permission,

    /// The Witness attesting that the Permission is a member of the
    /// accumulation.
    pub witness: Witness<Mpz>,

    /// The action being taken.
    pub action: Action,
}

/// A request to update an existing Permission by altering its actions.
#[derive(Deserialize, Serialize)]
pub struct UpdateRequest {

    /// The previous version of the Permission.
    pub perm: Permission,

    /// The Witness attesting that the previous version of the Permission
    /// is a member of the accumulation.
    pub witness: Witness<Mpz>,

    /// The new version of the Permission.
    pub update: Permission,
}

/// A response to the UpdateRequest.
#[derive(Deserialize, Serialize)]
pub struct UpdateResponse {

    /// The original UpdateRequest.
    pub req: UpdateRequest,

    /// The accumulation value after the Permision has been updated.
    pub value: Mpz,
}
