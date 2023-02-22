use clacc::{
    Accumulator,
    blake2::Map,
};
use gmp::mpz::Mpz;
use rand::RngCore;
use tokio::sync::Mutex;
use crate::{
    permission::Permission,
    request::{UpdateRequest, UpdateResponse, ActionRequest},
};

/// An Authority that controls the private key of an accumulator and is able
/// to add and delete Permissions.
pub struct Authority {

    /// The Accumulator's public key.
    key: Mpz,

    /// The Accumulator used to verify Permissions.
    verifying: Accumulator<Mpz, Map>,

    /// The Accumulator containing Permissions whose Witnesses are currently
    /// being updated by the Worker.
    updating: Accumulator<Mpz, Map>,

    /// The Accumulator containing the most recent versions of all
    /// Permissions.
    staging: Accumulator<Mpz, Map>,

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
        let mut rng = rand::thread_rng();
        let (acc, _, _) = Accumulator::<Mpz, Map>::with_random_key(
            |bytes| rng.fill_bytes(bytes),
            None,
        );
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
    pub fn get_key(&self) -> &Mpz {
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
        self.staging.add(perm.clone());
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
        match self.staging.del(req.perm.clone(), req.witness.clone()) {
            Err(_) => return Err("could not delete permission"),
            _ => (),
        };
        // Add the new Permission to the staging Accumulator.
        self.staging.add(req.update.clone());
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
        match self.verifying.verify(req.perm.clone(), req.witness.clone()) {
            Err(_) => return Err("could not verify permission"),
            _ => (),
        };
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
