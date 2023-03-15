use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::util::{DAY, HOUR, WEEK};

pub type Epoch = u64;
pub type Rank = f64;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Repo<'a> {
    #[serde(borrow)]
    pub remote: Cow<'a, str>,

    #[serde(borrow)]
    pub name: Cow<'a, str>,

    #[serde(borrow)]
    pub path: Cow<'a, str>,

    pub last_accessed: Epoch,
    pub accessed: Rank,
}

impl Repo<'_> {
    pub fn score(&self, now: Epoch) -> Rank {
        let duration = now.saturating_sub(self.last_accessed);
        if duration < HOUR {
            self.accessed * 4.0
        } else if duration < DAY {
            self.accessed * 2.0
        } else if duration < WEEK {
            self.accessed * 0.5
        } else {
            self.accessed * 0.25
        }
    }
}
