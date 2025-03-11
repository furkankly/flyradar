pub mod client;
pub mod errors;
pub mod proto;
mod set_sys_proc_attributes;
pub mod start;

use std::path::PathBuf;

use crate::config::helpers::get_config_directory;

pub fn path_to_socket() -> PathBuf {
    get_config_directory()
        .expect("Failed to get config directory")
        .join("fly-agent.sock")
}

#[derive(Debug, Clone)]
pub struct Instances {
    pub labels: Vec<String>,
    pub addresses: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_to_socket() {
        let socket_path = path_to_socket();
        assert!(socket_path.to_str().unwrap().contains("fly-agent.sock"));
    }
}
