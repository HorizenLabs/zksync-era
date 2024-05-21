use anyhow::Context as _;
use zksync_config::configs;
use zksync_protobuf::{repr::ProtoRepr, required};

use crate::proto::new_horizen as proto;

impl ProtoRepr for proto::NewHorizen {
    type Type = configs::NewHorizenConfig;
    fn read(&self) -> anyhow::Result<Self::Type> {
        Ok(Self::Type {
            url: required(&self.url)
                .and_then(|x| Ok((*x.as_str()).try_into()?))
                .context("url")?,
            seed_phrase: required(&self.seed_phrase)
                .and_then(|x| Ok((*x.as_str()).try_into()?))
                .context("seed_phrase")?,
        })
    }

    fn build(this: &Self::Type) -> Self {
        Self {
            url: Some(this.url.as_str().into()),
            seed_phrase: Some(this.seed_phrase.as_str().into()),
        }
    }
}
