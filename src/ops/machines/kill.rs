use color_eyre::eyre::eyre;

use crate::fly_rust::machines::{get_machine, kill_machine};
use crate::ops::Ops;
use crate::state::RdrResult;

#[derive(Debug)]
pub struct KillMachineInput {
    pub id: String,
}

pub async fn kill(ops: &Ops, app_name: &str, params: KillMachineInput) -> RdrResult<()> {
    let machine = get_machine(&ops.request_builder_machines, app_name, &params.id).await?;
    if machine.state == "destroyed" {
        return Err(eyre!("Machine {} has already been destroyed.", machine.id));
    }
    kill_machine(&ops.request_builder_machines, app_name, &machine.id).await?;

    Ok(())
}
