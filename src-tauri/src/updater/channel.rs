#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Release,
    Prerelease,
}

impl Channel {
    /// The channel baked into this binary at compile time by CI via the
    /// `PIXEL_BUILD_MANAGER_CHANNEL` env var (see the `package` job in
    /// `.github/workflows/pixel-build-manager-build.yml`). `None` means the
    /// var was unset or unrecognized -- e.g. a local `cargo build` outside
    /// CI -- and callers must treat that as "skip update checks", never
    /// guess a channel.
    pub fn current() -> Option<Channel> {
        Self::from_env_value(option_env!("PIXEL_BUILD_MANAGER_CHANNEL"))
    }

    fn from_env_value(value: Option<&str>) -> Option<Channel> {
        match value {
            Some("release") => Some(Channel::Release),
            Some("prerelease") => Some(Channel::Prerelease),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_value_parses_release() {
        assert_eq!(Channel::from_env_value(Some("release")), Some(Channel::Release));
    }

    #[test]
    fn from_env_value_parses_prerelease() {
        assert_eq!(
            Channel::from_env_value(Some("prerelease")),
            Some(Channel::Prerelease)
        );
    }

    #[test]
    fn from_env_value_is_none_when_unset() {
        assert_eq!(Channel::from_env_value(None), None);
    }

    #[test]
    fn from_env_value_is_none_for_unrecognized_values() {
        assert_eq!(Channel::from_env_value(Some("nightly")), None);
    }
}
