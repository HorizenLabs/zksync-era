use zksync_config::configs::NewHorizenConfig;

use crate::{envy_load, FromEnv};

impl FromEnv for NewHorizenConfig {
    fn from_env() -> anyhow::Result<Self> {
        envy_load("new_horizen", "NEW_HORIZEN_")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::EnvMutex;

    static MUTEX: EnvMutex = EnvMutex::new();

    fn expected_config() -> NewHorizenConfig {
        NewHorizenConfig {
            url: "ws://127.0.0.1:9944".to_string(),
            seed_phrase: "test test test".to_string(),
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
            NEW_HORIZEN_URL="ws://127.0.0.1:9944"
            NEW_HORIZEN_SEED_PHRASE="test test test"
        "#;
        let mut lock = MUTEX.lock();
        lock.set_env(config);
        let actual = NewHorizenConfig::from_env().unwrap();
        assert_eq!(actual, expected_config());
    }
}
