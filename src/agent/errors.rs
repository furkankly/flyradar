use std::fmt;

#[derive(Debug, Clone)]
pub enum AgentError {
    NoSuchHost,
    TunnelUnavailable,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::NoSuchHost => write!(f, "host was not found in DNS"),
            AgentError::TunnelUnavailable => write!(f, "tunnel unavailable"),
        }
    }
}
