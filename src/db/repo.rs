use std::borrow::Cow;

use serde::{Deserialize, Serialize};

pub type Epoch = u64;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Repo<'a> {
    #[serde(borrow)]
    pub name: Cow<'a, str>,

    #[serde(borrow)]
    pub path: Cow<'a, str>,

    pub last_accessed: Epoch,
    pub accessed: u64,
}
