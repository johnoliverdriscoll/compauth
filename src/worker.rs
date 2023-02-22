use clacc::{
    Accumulator,
    Update,
    Witness,
    blake2::Map,
};
use gmp::mpz::Mpz;
use crossbeam::thread;
use num_cpus;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex as StdMutex},
};
use tokio::sync::Mutex;
use crate::{
    permission::{Nonce, Permission},
    request::UpdateResponse,
};

/// Type for a map where Nonces map to Permission-Witness pairs.
type PermissionMap = HashMap<Nonce, (Permission, Witness<Mpz>)>;

/// A Worker that absorbs new and update Permissions during a window and can
/// perform a batched Update on a set of Witnesses.
pub struct Worker {

    /// The value of the Accumulator before any updates absorbed during the
    /// current window have been applied.
    value: Mpz,

    /// The current Accumulator. Although it will only have the public key,
    /// it will stay synchronized with the trusted value.
    ///
    /// The field will start in a None state until the public key from the
    /// Authority can be set using `set_key`.
    acc: Option<Accumulator<Mpz, Map>>,

    /// The absorbed updates.
    update: Update<Mpz, Map>,

    /// The Permission-Witness pairs that will be added during the current
    /// update window.
    additions: PermissionMap,

    /// The current map of Permission-Witness pairs.
    perms: PermissionMap,

    /// The additions that are having their initial witnesses calculated
    /// during the update process.
    updating_additions: PermissionMap,

    /// The permissions that are having their witnesses updated during the
    /// update process.
    updating_perms: PermissionMap,

    /// Mutex locked during updates to the Accumulator.
    guard_acc: Mutex<()>,

    /// Mutex locked while the Worker is in the process of updating Witnesses.
    guard_update: Mutex<()>,
}

impl Worker {

    /// Create a new Worker using the public key returned from the Authority.
    pub fn new() -> Self {
        // Allocate Worker.
        Worker {
            value: 0.into(),
            acc: None,
            update: Update::new(),
            additions: HashMap::new(),
            perms: HashMap::new(),
            updating_additions: HashMap::new(),
            updating_perms: HashMap::new(),
            guard_acc: Mutex::new(()),
            guard_update: Mutex::new(()),
        }
    }

    /// Submit the Authority's public key.
    ///
    /// This allocates the Worker's Accumulator and allows the other methods
    /// to be called successfully. If there is already an Accumulator
    /// allocated, this method returns an error.
    pub async fn set_key(
        &mut self,
        key: Mpz,
    ) -> Result<(), &'static str> {
        // Lock the Accumulator Mutex.
        let _guard_acc = self.guard_acc.lock().await;
        // Error out if there is already an Accumulator allocated.
        match self.acc {
            Some(_) => Err("already have public key"),
            None => {
                // Allocate new Accumulator initialized from the Authority's
                // public key.
                let acc = Accumulator::<Mpz, Map>::with_public_key(key);
                self.value = acc.get_value().clone();
                self.acc = Some(acc);
                Ok(())
            }
        }
    }

    /// Internal helper to add a new permission.
    ///
    /// This code is reused by `add_permission` and `update_permission`.
    /// It is assumed that the caller has locked a Mutex so that operations on
    /// the Accumulator are thread safe.
    fn add_permission_internal(
        perm: Permission,
        value: &Mpz,
        acc: &mut Accumulator<Mpz, Map>,
        update: &mut Update<Mpz, Map>,
        additions: &mut PermissionMap,
    ) {
        // Add Permission to Accumulator.
        let mut witness = acc.add(perm.clone());
        // Absorb the addition into the batched Update.
        update.add(perm.clone(), witness.clone());
        // Set the witness value.
        witness.set_value(value.clone());
        // Insert the pair into the collection of added elements.
        additions.insert(perm.nonce, (perm, witness));
    }

    /// Absorb a new Permission into the update window.
    pub async fn add_permission(
        &mut self,
        perm: Permission,
    ) -> Result<(), &'static str> {
        // Lock the Accumulator Mutex.
        let _guard_acc = self.guard_acc.lock().await;
        // Error out if there is no Accumulator allocated.
        let acc = match &mut self.acc {
            Some(acc) => acc,
            None => {
                return Err("need public key");
            },
        };
        // Use the helper to add the Permission.
        Self::add_permission_internal(
            perm,
            &self.value,
            acc,
            &mut self.update,
            &mut self.additions,
        );
        Ok(())
    }

    /// Absorb an updated Permission into the update window.
    ///
    /// This is simply a deletion of the old version and an addition of the
    /// new version.
    pub async fn update_permission(
        &mut self,
        res: UpdateResponse,
    ) -> Result<(), &'static str> {
        // Lock the Accumulator Mutex.
        let _guard_acc = self.guard_acc.lock().await;
        // Error out if there is no Accumulator allocated.
        let acc = match &mut self.acc {
            Some(acc) => acc,
            None => {
                return Err("need public key");
            },
        };
        // Absorb the deletion into the batched Update.
        self.update.del(res.req.perm.clone(), res.req.witness.clone());
        // Use the helper to add the Permission.
        Self::add_permission_internal(
            res.req.update,
            &self.value,
            acc,
            &mut self.update,
            &mut self.additions
        );
        // Synchronize the Worker's accumulation with the Authority's.
        // Note that the Worker can't call Accumulator.del because it does not
        // have the private key.
        acc.set_value(res.value);
        Ok(())
    }

    /// Retrieve the current Witness for a given Nonce.
    pub async fn witness(
        &self,
        nonce: Nonce,
    ) -> Result<Option<Witness<Mpz>>, &'static str> {
        // Lock the Accumulator Mutex to ensure latest Permissions collection
        // is available if called during the update process.
        let _guard_acc = self.guard_acc.lock().await;
        // Error out if there is no Accumulator allocated.
        match &self.acc {
            Some(_) => {},
            None => {
                return Err("need public key");
            },
        }
        // Return the Witness stored for the Nonce.
        match self.perms.get(&nonce) {
            Some(pair) => Ok(Some(pair.1.clone())),
            None => Ok(None),
        }
    }

    /// Perform Witness updates.
    ///
    /// This will block the current thread during the process, however, other
    /// threads may call `add_permission` and `update_permission` to absorb
    /// updates for the next window without adversely affecting the current
    /// update process.
    pub async fn update(&mut self) -> Result<(), &'static str> {
        // Lock the update Mutex.
        let _guard_update = self.guard_update.lock().await;
        // Error out if there is no Accumulator allocated.
        match self.acc {
            Some(_) => {},
            None => {
                return Err("need public key");
            },
        }
        // Cache volatile values that are needed for the update process.
        let acc;
        let update;
        {
            // Lock the Accumulator Mutex so that other threads cannot call
            // `add_permission` or `update_permission` while the instance
            // values are copied to the local cache.
            let _guard_acc = self.guard_acc.lock().await;
            // Store a copy of the current Accumulator.
            acc = self.acc.as_ref().unwrap().clone();
            // Store a copy of the updates absorbed during this update window.
            update = self.update.clone();
            // Copy the elements added during this update window.
            self.updating_additions = self.additions.clone();
            // Reset the batched Update and clear the additions collection
            // for subsequent calls to `add_permission` and
            // `update_permission`.
            self.update = Update::new();
            self.additions.clear();
            // Set the accumulation value for the additions in the next
            // update.
            self.value = acc.get_value().clone();
            // The Accumulator Mutex gets unlocked here, allowing other
            // threads to call `add_permission` or `update_permission`.
        }
        // Update witnesses.
        let additions = Arc::new(StdMutex::new(
            self.updating_additions.values_mut()
        ));
        let staticels = Arc::new(StdMutex::new(
            self.updating_perms.values_mut()
        ));
        thread::scope(|scope| {
            for _ in 0..num_cpus::get() {
                let acc = acc.clone();
                let u = update.clone();
                let additions = Arc::clone(&additions);
                let staticels = Arc::clone(&staticels);
                scope.spawn(move |_| u.update_witnesses(
                    &acc,
                    additions,
                    staticels,
                ));
            }
            Ok(())
        }).unwrap()
    }

    /// Finalize the update process.
    pub async fn sync(&mut self) {
        // Lock the update Mutex.
        let _guard_update = self.guard_update.lock().await;
        // Insert the Permissions that were added during this update window
        // into the updated Permissions map.
        for pair in self.updating_additions.values() {
            self.updating_perms.insert(pair.0.nonce, pair.clone());
        }
        // Lock the Accumulator Mutex so that other threads may not call
        // `add_permission` or `update_permission` while the updated
        // Permissions map is copied back into the `perms` field.
        let _guard_acc = self.guard_acc.lock().await;
        // Copy the updated Permissions map into the `perms` field.
        self.perms = self.updating_perms.clone();
    }
}
