use std::collections::{HashMap, HashSet};

use color_eyre::eyre::OptionExt;

use crate::ops::lease::list_active_machines;
use crate::ops::Ops;
use crate::state::RdrResult;

pub async fn services(ops: &Ops, app_name: String) -> RdrResult<()> {
    let mut services: HashSet<String> = HashSet::new();
    let mut service_to_process_group: HashMap<String, Vec<String>> = HashMap::new();
    let mut service_to_region: HashMap<String, Vec<String>> = HashMap::new();
    let mut service_to_machines: HashMap<String, i32> = HashMap::new();

    let machines = list_active_machines(&ops.request_builder_machines, &app_name).await?;

    for (machine, service, port) in machines
        .iter()
        .filter_map(|machine| machine.config.as_ref().map(|config| (machine, config)))
        .filter_map(|(machine, config)| {
            config.services.as_ref().map(|services| (machine, services))
        })
        .flat_map(|(machine, services)| services.iter().map(move |service| (machine, service)))
        .filter_map(|(machine, service)| {
            service
                .ports
                .as_ref()
                .map(|ports| (machine, service, ports))
        })
        .flat_map(|(machine, service, ports)| {
            ports.iter().map(move |port| (machine, service, port))
        })
    {
        let protocol = &service.protocol;
        // Get port number or skip this iteration if None
        let port_num = port.port.ok_or_eyre("No port number is found.")?;
        let ports = format!("{} => {}", port_num, service.internal_port);
        let mut https = port.force_https.unwrap_or_default().to_string();
        make_ascii_titlecase(&mut https);
        let handlers = port
            .handlers
            .as_ref()
            .map(|h| {
                h.iter()
                    .map(|handler| handler.to_uppercase())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
            .join(",");

        let key = get_service_key(protocol, ports, &https, &handlers);
        services.insert(key.clone());

        service_to_process_group
            .entry(key.clone())
            .or_default()
            .push(machine.process_group());
        service_to_region
            .entry(key.clone())
            .or_default()
            .push(machine.region.clone());
        *service_to_machines.entry(key).or_insert(0) += 1;
    }

    let mut shared_state_guard = ops.shared_state.lock().unwrap();
    shared_state_guard.app_services_list = services
        .iter()
        .map(|service| {
            let components: Vec<&str> = service.split('-').collect();
            vec![
                components[0].to_uppercase(),
                components[1].to_uppercase(),
                format!("[{}]", components[3].to_uppercase()),
                components[2].to_string(),
                service_to_process_group
                    .get_mut(service)
                    .map(|process_groups| {
                        process_groups.sort();
                        process_groups.dedup();
                        process_groups.join(",")
                    })
                    .unwrap_or_default(),
                service_to_region
                    .get_mut(service)
                    .map(|regions| {
                        regions.sort();
                        regions.dedup();
                        regions.join(",")
                    })
                    .unwrap_or_default(),
                service_to_machines
                    .get(service)
                    .map(|count| count.to_string())
                    .unwrap_or_default(),
            ]
        })
        .collect();
    Ok(())
}

fn get_service_key(protocol: &str, ports: String, forcehttps: &str, handlers: &str) -> String {
    format!("{}-{}-{}-{}", protocol, ports, forcehttps, handlers)
}

fn make_ascii_titlecase(s: &mut str) {
    if let Some(r) = s.get_mut(0..1) {
        r.make_ascii_uppercase();
    }
}
