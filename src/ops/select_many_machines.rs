use futures::future::try_join_all;

use super::Ops;
use crate::fly_rust::machine_types::Machine;
use crate::fly_rust::machines::get_machine;
use crate::state::RdrResult;

pub async fn select_many_machines(
    ops: &Ops,
    app_name: &str,
    machine_ids: Vec<String>,
) -> RdrResult<Vec<Machine>> {
    try_join_all(
        machine_ids
            .iter()
            .map(|id| get_machine(&ops.request_builder_machines, app_name, id)),
    )
    .await
}
