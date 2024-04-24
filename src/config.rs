use crate::proto::{Peer, PrivateKey, Protocol, PublicKey};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use std::collections::{HashMap, HashSet};
use toml::Value;

pub type TransportConfig = HashMap<String, Value>;

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde_as(as = "DisplayFromStr")]
    pub public_key: PublicKey,
    #[serde_as(as = "DisplayFromStr")]
    pub private_key: PrivateKey,
    #[serde(default)]
    pub enabled_transports: HashSet<Protocol>,
    #[serde(default)]
    pub transport_configs: HashMap<Protocol, TransportConfig>,
    #[serde(default)]
    #[serde_as(as = "HashSet<DisplayFromStr>")]
    pub seeders: HashSet<Peer>,
    #[serde(default)]
    #[serde_as(as = "HashSet<DisplayFromStr>")]
    pub deny_list: HashSet<Peer>,
}
