use std::str::FromStr;

use color_eyre::eyre::{eyre, Error};

use crate::state::RdrResult;

pub const COMMANDS: &[&str] = &["apps", "machines", "volumes", "secrets", "q!"];

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Command {
    Apps,
    Machines,
    Volumes,
    Secrets,
    Quit,
}

impl FromStr for Command {
    type Err = Error;

    fn from_str(s: &str) -> RdrResult<Self> {
        match s {
            "a" | "app" | "apps" => Ok(Self::Apps),
            "m" | "machine" | "machines" => Ok(Self::Machines),
            "v" | "vol" | "volume" | "volumes" => Ok(Self::Volumes),
            "s" | "secret" | "secrets" => Ok(Self::Secrets),
            "q" | "q!" => Ok(Self::Quit),
            _ => Err(eyre!("Unknown command: {}", s)),
        }
    }
}
pub fn match_command(s: &str) -> &str {
    COMMANDS
        .iter()
        .find(|cmd| !s.is_empty() && cmd.len() > s.len() && cmd.starts_with(s))
        .copied()
        .unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_matching() {
        assert_eq!(match_command("a"), "apps");
        assert_eq!(match_command("m"), "machines");
        assert_eq!(match_command("vo"), "volumes");
        assert_eq!(match_command("secr"), "secrets");
        assert_eq!(match_command("q"), "q!");
        assert_eq!(match_command("invalid"), "invalid");
    }
}
