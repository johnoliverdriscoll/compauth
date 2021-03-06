use clacc::{
    Accumulator,
    blake2::Mapper as M,
    gmp::BigInt,
    velocypack::VpackSerializer,
    typenum::U16 as N,
};
use tokio::sync::Mutex;
use crate::permission::Permission;
use crate::request::{UpdateRequest, UpdateResponse, ActionRequest};

/// Type for a VelocyPack serialized Permission.
type S = VpackSerializer<Permission>;

/// An Authority that controls the private key of an accumulator and is able
/// to add and delete Permissions.
pub struct Authority {

    /// The Accumulator's public key.
    key: BigInt,

    /// The Accumulator used to verify Permissions.
    verifying: Accumulator<BigInt>,

    /// The Accumulator containing Permissions whose Witnesses are currently
    /// being updated by the Worker.
    updating: Accumulator<BigInt>,

    /// The Accumulator containing the most recent versions of all
    /// Permissions.
    staging: Accumulator<BigInt>,

    /// Mutex locked while the Authority is operating on its Accumulators.
    guard: Mutex<()>,
}

impl Authority {

    /// Create a new Authority.
    pub fn new() -> Self {
        // Generate an accumulator. In a real world scenario, the
        // Accumulator's private key would be generated and sharded as part of
        // a key ceremony. Security officers entrusted with the shards would
        // then submit their part of the key to the Authority server in order
        // to reconstitute in a secure hardware environment. This is out-of-
        // scope for the purposes of this demonstration, so an Accumulator is
        // instead initialized from a random private key.
        let (acc, _, _) = Accumulator::<BigInt>::with_random_key(None);
        // Allocate the Authority using the public key and three copies of the
        // Accumulator for each phase of the update process.
        Authority {
            key: acc.get_public_key().clone(),
            verifying: acc.clone(),
            updating: acc.clone(),
            staging: acc.clone(),
            guard: Mutex::new(()),
        }
    }

    /// Return the Accumulator's public key.
    pub fn get_key(&self) -> &BigInt {
        &self.key
    }

    /// Add a Permission.
    pub async fn add_permission(
        &mut self,
        mut perm: Permission,
    ) -> Permission {
        // Lock the Mutex.
        let _guard = self.guard.lock().await;
        // Assign a random Nonce that prevents other Permissions from
        // overwriting this Permission in the future.
        perm.nonce = rand::random::<u64>().into();
        // Add the Permission to the staging Accumulator.
        self.staging.add::<M, N, S, _>(&perm);
        // Return the Permission with the new Nonce.
        perm
    }

    /// Update an existing Permission.
    pub async fn update_permission(
        &mut self,
        req: UpdateRequest,
    ) -> Result<UpdateResponse, &'static str> {
        // Ensure the new Permission's Nonce matches the old Permission's
        // Nonce.
        if req.update.nonce != req.perm.nonce {
            return Err("nonce mismatch");
        }
        // Ensure the new Permission's version is greater than the old
        // Permission's version.
        if req.update.version <= req.perm.version {
            return Err("new version must be greater than old version");
        }
        // Lock the Mutex.
        let _guard = self.guard.lock().await;
        // Delete the old Permission from the staging Accumulator.
        self.staging.del::<M, N, S, _>(&req.perm, &req.witness)?;
        // Add the new Permission to the staging Accumulator.
        self.staging.add::<M, N, S, _>(&req.update);
        // Return the latest accumulation value.
        Ok(UpdateResponse {
            req: req,
            value: self.staging.get_value().clone(),
        })
    }

    /// Perform an action if a given Permission is part of the Accumulation.
    pub async fn action(
        &self,
        req: ActionRequest,
    ) -> Result<(), &'static str> {
        // Lock the Mutex.
        let _guard = self.guard.lock().await;
        // Verify the Permission is part of the verifying Accumulator.
        self.verifying.verify::<M, N, S, _>(&req.perm, &req.witness)?;
        // Ensure the requested action is in the actions list.
        match req.perm.actions.iter().find(|&action| action == &req.action) {
            Some(_) => Ok(()),
            None => Err("permission not granted to perform action"),
        }
    }

    /// Copy the current staging Accumulator to the updating Accumulator.
    ///
    /// This should be called when the Worker begins updating Witnesses so
    /// that the updating Accumulator captures all Permission additions and
    /// deletions made during the update window.
    pub async fn update(&mut self) {
        let _guard = self.guard.lock().await;
        self.updating = self.staging.clone();
    }

    /// Copy the current updating Accumulator to the verifying Accumulator.
    ///
    /// This should be called when the Worker has finished updating all
    /// Witnesses so that the verifying Accumulator reflects all additions
    /// and deletions that have been captured during the previous update
    /// window.
    pub async fn sync(&mut self) {
        let _guard = self.guard.lock().await;
        self.verifying = self.updating.clone();
    }
}
