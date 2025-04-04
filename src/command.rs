use std::str::FromStr;

use color_eyre::eyre::{eyre, Error};

use crate::state::RdrResult;

pub const COMMANDS: &[&str] = &[
    "organizations",
    "apps",
    "machines",
    "volumes",
    "secrets",
    "quit",
];

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Command {
    Organizations,
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
            "o" | "org" | "orgs" | "organizations" => Ok(Self::Organizations),
            "a" | "app" | "apps" => Ok(Self::Apps),
            "m" | "mac" | "machine" | "machines" => Ok(Self::Machines),
            "v" | "vol" | "volume" | "volumes" => Ok(Self::Volumes),
            "s" | "sec" | "secret" | "secrets" => Ok(Self::Secrets),
            "q" | "q!" | "quit" => Ok(Self::Quit),
            _ => Err(eyre!("Unknown command: {}", s)),
        }
    }
}

impl Command {
    pub fn to_aliases(&self) -> &[&'static str] {
        match self {
            Command::Organizations => &["o", "org", "orgs", "organizations"],
            Command::Apps => &["a", "app", "apps"],
            Command::Machines => &["m", "mac", "machine", "machines"],
            Command::Volumes => &["v", "vol", "volume", "volumes"],
            Command::Secrets => &["s", "sec", "secret", "secrets"],
            Command::Quit => &["q", "q!", "quit"],
        }
    }
}

pub fn match_command(s: &str) -> &str {
    if s.is_empty() {
        return s;
    }

    COMMANDS
        .iter()
        .find_map(|&cmd_str| {
            // Try to parse each command
            cmd_str.parse::<Command>().ok().and_then(|cmd| {
                // For each command, find the first alias that is a prefix match and longer
                cmd.to_aliases()
                    .iter()
                    .find(|&&alias| alias.starts_with(s) && alias.len() > s.len())
                    .copied()
            })
        })
        .unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_matching() {
        assert_eq!(match_command("o"), "organizations");
        assert_eq!(match_command("a"), "apps");
        assert_eq!(match_command("m"), "machines");
        assert_eq!(match_command("vo"), "volumes");
        assert_eq!(match_command("secr"), "secrets");
        assert_eq!(match_command("q"), "q!");
        assert_eq!(match_command("invalid"), "invalid");
    }
}
