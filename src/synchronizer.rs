use clacc::{Witness, gmp::BigInt};
use hyper::body::to_bytes;
use std::sync::atomic::AtomicPtr;
use tokio::{sync::Mutex, task::JoinHandle, time::{interval, Duration}};
use crate::{
    constant::{AUTHORITY_ADDR, WORKER_ADDR, UPDATE_WINDOW_MILLIS},
    permission::{Action, Nonce, Permission},
    request::{ActionRequest, UpdateRequest, UpdateResponse},
    util::{from_bytes, Client},
};

/// A Synchronizer manages the Witness update window by synchronizing
/// the Authority and the Worker.
///
/// Requests to the system need to go through the Synchronizer to
/// eliminate possible race conditions during the update process.
pub struct Synchronizer {
    auth_client: Client,
    worker_client: Client,
    guard_acc: Mutex<()>,
    guard_update: Mutex<()>,
}

impl Synchronizer {

    /// Create a new Synchronizer.
    pub async fn new() -> Result<Self, &'static str> {
        Synchronizer {
            auth_client: Client::new(AUTHORITY_ADDR),
            worker_client: Client::new(WORKER_ADDR),
            guard_acc: Mutex::new(()),
            guard_update: Mutex::new(()),
        }.key_worker().await
    }

    /// Set the Worker's public key by requesting it from the Authority.
    async fn key_worker(mut self) -> Result<Self, &'static str> {
        // Request the public key from the Authority.
        let resp = self.auth_client.get("/key").await?;
        // Deserialize the response to a BigInt.
        let bytes = to_bytes(resp.into_body()).await;
        let key: BigInt = match from_bytes(&bytes) {
            Some(res) => res,
            None => {
                return Err("response error");
            },
        };
        // Submit the public key to the Worker.
        self.worker_client.post("/key", key).await?;
        // Return self on success.
        Ok(self)
    }

    /// Add a permission to the system.
    pub async fn add_permission(
        &mut self,
        actions: Vec<Action>,
    ) -> Result<Permission, &'static str> {
        // Lock the Mutex.
        let _guard = self.guard_acc.lock().await;
        // Create a Permission that includes the requested actions.
        let mut perm = Permission {
            nonce: 0.into(),
            actions: actions,
            version: 0,
        };
        // Submit the permission to the Authority and read back the response
        // that includes populated Nonce.
        let resp = self.auth_client.post("/permission", perm).await?;
        let bytes = to_bytes(resp.into_body()).await;
        perm = match from_bytes(&bytes) {
            Some(res) => res,
            None => {
                return Err("response error");
            },
        };
        // Submit the finalized Permission to the Worker.
        self.worker_client.post("/permission", perm.clone()).await?;
        // Return the Permission on success.
        Ok(perm)
    }

    /// Internal helper to get the witness for a Permission.
    ///
    /// This code is reused by `update_permission` and `action` so that a
    /// current witness can be attached to the request to the Authority.
    async fn get_witness(
        worker_client: &mut Client,
        nonce: Nonce,
    ) -> Result<Witness<BigInt>, &'static str> {
        // Build the request path in the form of "/witness/{nonce}".
        let mut path = "/witness/".to_owned();
        path.push_str(&nonce.to_string());
        // Request the path from the Worker and deserialize the response.
        let resp = worker_client.get(&path).await?;
        let bytes = to_bytes(resp.into_body()).await;
        match from_bytes::<Witness<BigInt>, _>(&bytes) {
            Some(res) => Ok(res),
            None => Err("response error"),
        }
    }

    /// Update a permission.
    pub async fn update_permission(
        &mut self,
        perm: Permission,
        actions: Vec<Action>
    ) -> Result<Permission, &'static str> {
        // Lock the Mutex.
        let _guard = self.guard_acc.lock().await;
        // Get the Permission's current Witness.
        let witness = Self::get_witness(
            &mut self.worker_client,
            perm.nonce
        ).await?;
        // Create Permission with new actions and an incremented version.
        let update = Permission {
            nonce: perm.nonce,
            actions: actions,
            version: perm.version + 1,
        };
        // Create the UpdateRequest struct containing the Witness as well as
        // the old and new Permissions.
        let req = UpdateRequest {
            perm: perm,
            witness: witness,
            update: update.clone(),
        };
        // Submit the request to the Authority and deserialize the response.
        let resp = self.auth_client.put("/permission", req).await?;
        let bytes = to_bytes(resp.into_body()).await;
        let response: UpdateResponse = match from_bytes(&bytes) {
            Some(res) => res,
            None => {
                return Err("response error");
            },
        };
        // Submit the response to the Worker so that it has the most current
        // accumulation value.
        self.worker_client.put("/permission", response).await?;
        // Return the updated Permission on success.
        Ok(update)
    }

    /// Perform an action.
    pub async fn action(
        &mut self,
        perm: Permission,
        action: Action,
    ) -> Result<(), &'static str> {
        // Lock the Mutex.
        let _guard = self.guard_acc.lock().await;
        // Get the Permission's current Witness.
        let witness = Self::get_witness(
            &mut self.worker_client,
            perm.nonce
        ).await?;
        // Create the ActionRequest struct.
        let req = ActionRequest {
            perm: perm,
            witness: witness,
            action: action,
        };
        // Submit the request.
        self.auth_client.post("/action", req).await?;
        // Return success.
        Ok(())
    }

    /// Start the synchronization task.
    ///
    /// The synchronization task executes in a continuous loop until a
    /// communication error occurs with the Authority or the Worker.
    /// The owner of a Synchronizer instance must await the returned future
    /// before the instance may be freed safely.
    pub fn sync(&mut self) -> JoinHandle<Result<(), &'static str>> {
        // Create an AtomicPtr so that a reference to the instance may be moved
        // into the task.
        let mut ptr = AtomicPtr::new(self);
        tokio::spawn(async move {
            // Dereference the pointer to get the refenence.
            let sync = unsafe {
                ptr.get_mut().as_mut().unwrap()
            };
            // Lock the update Mutex to prevent additional sync tasks from
            // executing.
            let _guard_update = sync.guard_update.lock().await;
            // Define the update window.
            let dur = Duration::from_millis(UPDATE_WINDOW_MILLIS);
            let mut window = interval(dur);
            // The first tick completes immediately. Get it out of the way.
            window.tick().await;
            // Start looping.
            loop {
                // Wait for the next interval tick.
                window.tick().await;
                // Create a future for the update task, but only lock the
                // Accumulator Mutex while the Authority and Worker states
                // are mutated.
                {
                    // Lock the accumulator Mutex.
                    let _guard_acc = sync.guard_acc.lock().await;
                    // Tell the Authority to switch over its staging
                    // accumulation.
                    sync.auth_client.get("/update").await?;
                    // Tell the Worker to start updating Witnesses.
                    sync.worker_client.get("/update")
                    // Mutex gets released here, even though the Worker update
                    // result will be awaited for.
                }.await?;
                // Lock the accumulator Mutex.
                let _guard_acc = sync.guard_acc.lock().await;
                // Tell the Authority to switch over its updating accumulation.
                sync.auth_client.get("/sync").await?;
                // Tell the Worker to switch over its permissions map.
                sync.worker_client.get("/sync").await?;
            }
        })
    }
}
