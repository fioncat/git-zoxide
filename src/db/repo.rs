use std::borrow::Cow;

use serde::{Deserialize, Serialize};

pub type Epoch = u64;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Repo<'a> {
    pub remote: Cow<'a, str>,
    pub group: Cow<'a, str>,
    pub name: Cow<'a, str>,

    pub path: Cow<'a, str>,

    pub last_accessed: Epoch,
}
