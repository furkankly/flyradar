use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use color_eyre::eyre::{eyre, Report};
use futures::future::{BoxFuture, FutureExt};
use futures::stream::{
    StreamExt, {self},
};

use super::Ops;
use crate::fly_rust::machine_types::{HostStatus, Machine};
use crate::fly_rust::machines::{acquire_lease, get_machine, list_machines, release_lease};
use crate::fly_rust::request_builder::RequestBuilderMachines;
use crate::state::RdrResult;

const MAX_CONCURRENT_LEASES: usize = 20;

type ReleaseFuture = BoxFuture<'static, ()>;
pub struct ReleaseGuard<F: Future<Output = ()> + std::marker::Send + 'static> {
    pub release: Option<F>,
}

impl<F: Future<Output = ()> + std::marker::Send + 'static> Drop for ReleaseGuard<F> {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            tokio::spawn(release);
        }
    }
}

pub async fn list_active_machines(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
) -> RdrResult<Vec<Machine>> {
    let machines = list_machines::<Machine>(request_builder_machines, app_name, false).await?;
    Ok(machines
        .into_iter()
        .filter(|m| {
            m.config.is_some()
                && m.is_active()
                && !m.is_release_command_machine()
                && !m.is_fly_apps_console()
        })
        .collect())
}

async fn _acquire_all_leases(
    ops: &Ops,
    app_name: &str,
) -> (Vec<Arc<Mutex<Machine>>>, Vec<Arc<Report>>, ReleaseFuture) {
    match list_active_machines(&ops.request_builder_machines, app_name).await {
        Ok(machines) => acquire_leases(ops, app_name, machines).await,
        Err(_error) => {
            let no_op_release: ReleaseFuture = Box::pin(async {});
            (vec![], vec![], no_op_release)
        }
    }
}

type MachineLeaseAcquireResult = Result<Arc<Mutex<Machine>>, Arc<Report>>;
pub async fn acquire_leases(
    ops: &Ops,
    app_name: &str,
    machines: Vec<Machine>,
) -> (Vec<Arc<Mutex<Machine>>>, Vec<Arc<Report>>, ReleaseFuture) {
    let machines: Vec<Arc<Mutex<Machine>>> = machines
        .into_iter()
        .map(|machine| Arc::new(Mutex::new(machine)))
        .collect();

    let results: Vec<MachineLeaseAcquireResult> = stream::iter(machines)
        .map(|m| async move {
            {
                let machine = m.lock().unwrap();
                // Skip lease acquisition for unreachable machines
                if machine.host_status != HostStatus::Ok {
                    return Ok(m.clone());
                }
            }
            match acquire(ops, app_name, m.clone()).await {
                Ok((updated_machine, _release_fn)) => Ok(updated_machine),
                Err(e) => {
                    let machine = m.lock().unwrap();
                    let error = Arc::new(eyre!(
                        "Failed to acquire lease for machine {}: {}",
                        machine.id,
                        e
                    ));
                    Err(error)
                }
            }
        })
        .buffer_unordered(MAX_CONCURRENT_LEASES)
        .collect::<Vec<_>>()
        .await;

    let lease_holding_machines: Vec<Arc<Mutex<Machine>>> = results
        .iter()
        .filter_map(|r| r.as_ref().ok().cloned())
        .collect();

    let errors: Vec<Arc<Report>> = results
        .iter()
        .filter_map(|r| r.as_ref().err().cloned())
        .collect();

    let lease_holding_machines_clone = lease_holding_machines.clone();
    let app_name_clone = app_name.to_string();
    let request_builder_machines_clone = ops.request_builder_machines.clone();
    let release = async move {
        stream::iter(lease_holding_machines_clone)
            .map(move |machine| {
                let app_name_clone = app_name_clone.clone();
                let request_builder_machines_clone = request_builder_machines_clone.clone();
                async move {
                    let (id, lease_nonce) = {
                        let machine = machine.lock().unwrap();
                        (machine.id.clone(), machine.lease_nonce.clone())
                    };
                    let result = release(
                        &request_builder_machines_clone,
                        &app_name_clone,
                        id.clone(),
                        lease_nonce,
                    )
                    .await;
                    (id, result)
                }
            })
            .buffer_unordered(MAX_CONCURRENT_LEASES)
            .collect::<Vec<_>>()
            .await;
    }
    .boxed();

    (lease_holding_machines, errors, release)
}

async fn release(
    request_builder_machines: &RequestBuilderMachines,
    app_name: &str,
    machine_id: String,
    nonce: Option<String>,
) -> RdrResult<()> {
    if let Some(lease_nonce) = nonce {
        release_lease(
            request_builder_machines,
            app_name,
            machine_id.as_str(),
            lease_nonce.as_str(),
        )
        .await
        .map_err(|e| {
            if !e.to_string().contains("lease not found") {
                return eyre!("failed to release lease for machine {}: {}", machine_id, e);
            }
            e
        })
    } else {
        Ok(())
    }
}

pub async fn acquire(
    ops: &Ops,
    app_name: &str,
    machine: Arc<Mutex<Machine>>,
) -> RdrResult<(Arc<Mutex<Machine>>, ReleaseFuture)> {
    let machine_id = machine.lock().unwrap().id.clone();

    let message = format!("Waiting on lease for machine {}", machine_id);
    let feedback_tx = ops.show_delayed_feedback(message, Duration::from_secs(2));

    let lease = acquire_lease(
        &ops.request_builder_machines,
        app_name,
        &machine_id,
        Some(120),
    )
    .await
    .map_err(|e| eyre!("failed to obtain lease: {}", e))?;

    drop(feedback_tx); // Cancel the feedback

    let lease_data = lease.data.expect("lease_data_not_found");
    let instance_id = {
        let mut machine_guard = machine.lock().unwrap();
        machine_guard.lease_nonce = Some(String::from(&lease_data.nonce));
        machine_guard.instance_id.clone()
    };
    let machine_clone = Arc::clone(&machine);
    let app_name_clone = app_name.to_string();
    let request_builder_machines_clone = ops.request_builder_machines.clone();
    let release_future: ReleaseFuture = async move {
        let (id, lease_nonce) = {
            let machine = machine_clone.lock().unwrap();
            (machine.id.clone(), machine.lease_nonce.clone())
        };
        let _ = release(
            &request_builder_machines_clone,
            &app_name_clone,
            id,
            lease_nonce,
        )
        .await;
    }
    .boxed();

    if instance_id == lease_data.version {
        Ok((machine, release_future))
    } else {
        match get_machine(&ops.request_builder_machines, app_name, &machine_id).await {
            Ok(mut updated_machine) => {
                updated_machine.lease_nonce = Some(lease_data.nonce);
                let updated_machine = Arc::new(Mutex::new(updated_machine));
                // let a = updated_machine.lock().unwrap();
                // *a = updated_machine;
                let updated_machine_clone = Arc::clone(&updated_machine);
                let app_name_clone = String::from(app_name);
                let request_builder_machines_clone = ops.request_builder_machines.clone();
                let final_release_fn = async move {
                    let (id, lease_nonce) = {
                        let machine = updated_machine_clone.lock().unwrap();
                        (machine.id.clone(), machine.lease_nonce.clone())
                    };
                    let _ = release(
                        &request_builder_machines_clone,
                        &app_name_clone,
                        id,
                        lease_nonce,
                    )
                    .await;
                }
                .boxed();
                Ok((updated_machine, final_release_fn))
            }
            Err(_e) => Ok((machine, release_future)),
        }
    }
}
