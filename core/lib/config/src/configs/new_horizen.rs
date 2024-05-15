use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct NewHorizenConfig {
    pub url: String,
    pub seed_phrase: String,
}
