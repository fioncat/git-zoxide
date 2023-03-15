mod repo;

use std::{fs, io, path::PathBuf};

use anyhow::{bail, Context, Result};
use bincode::Options;
use ouroboros::self_referencing;

pub use crate::db::repo::{Epoch, Repo};
use crate::{config, util};

#[self_referencing]
pub struct Database {
    path: PathBuf,
    bytes: Vec<u8>,

    #[borrows(bytes)]
    #[covariant]
    pub repos: Vec<Repo<'this>>,
    dirty: bool,
}

impl Database {
    const VERSION: u32 = 1;

    pub fn open() -> Result<Database> {
        let data_dir = config::Config::get_data_dir()?;
        let path = data_dir.join("database");

        match fs::read(&path) {
            Ok(bytes) => Self::try_new(path, bytes, |bytes| Self::deserialize(bytes), false),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                fs::create_dir_all(&data_dir).with_context(|| {
                    format!("unable to create data directory: {}", data_dir.display())
                })?;
                Ok(Self::new(path, Vec::new(), |_| Vec::new(), false))
            }
            Err(err) => bail!("failed to read db: {err}"),
        }
    }

    pub fn save(&mut self) -> Result<()> {
        if !self.dirty() {
            return Ok(());
        }

        let bytes = Self::serialize(self.borrow_repos())?;
        if let Err(err) = util::write(self.borrow_path(), bytes) {
            bail!("failed to write database: {err}");
        }
        self.with_dirty_mut(|d| *d = false);

        Ok(())
    }

    pub fn add(
        &mut self,
        remote: impl AsRef<str> + Into<String>,
        name: impl AsRef<str> + Into<String>,
        path: impl AsRef<str> + Into<String>,
        now: Epoch,
    ) -> &Repo {
        self.with_repos_mut(|repos| {
            repos.push(Repo {
                remote: remote.into().into(),
                name: name.into().into(),
                path: path.into().into(),
                last_accessed: now,
                accessed: 0.0,
            });
        });
        match self.borrow_repos().iter().rev().next() {
            Some(repo) => repo,
            None => panic!("could not find the currently added repo"),
        }
    }

    pub fn get(
        &self,
        remote: impl AsRef<str> + Into<String>,
        name: impl AsRef<str> + Into<String>,
    ) -> Option<&Repo> {
        self.borrow_repos()
            .iter()
            .find(|repo| repo.remote == remote.as_ref() && repo.name == name.as_ref())
    }

    pub fn get_by_path(&self, path: impl AsRef<str> + Into<String>) -> Option<&Repo> {
        self.borrow_repos()
            .iter()
            .find(|repo| repo.path == path.as_ref())
    }

    pub fn get_latest(&self) -> Option<&Repo> {
        let mut result: Option<&Repo> = None;
        self.with_repos(|repos| {
            for repo in repos {
                match result {
                    None => result = Some(repo),
                    Some(cur) if repo.last_accessed > cur.last_accessed => result = Some(repo),
                    Some(_) => {}
                }
            }
        });
        result
    }

    pub fn contains(&self, query: impl AsRef<str> + Into<String>) -> Option<&Repo> {
        self.borrow_repos()
            .iter()
            .find(|repo| repo.name.contains(query.as_ref()))
    }

    pub fn contains_remote(
        &self,
        remote: impl AsRef<str> + Into<String>,
        query: impl AsRef<str> + Into<String>,
    ) -> Option<&Repo> {
        self.borrow_repos()
            .iter()
            .find(|repo| repo.remote == remote.as_ref() && repo.name.contains(query.as_ref()))
    }

    pub fn contains_remote_prefix(
        &self,
        remote: impl AsRef<str> + Into<String>,
        prefix: impl AsRef<str> + Into<String>,
        query: impl AsRef<str> + Into<String>,
    ) -> Option<&Repo> {
        self.borrow_repos().iter().find(|repo| {
            if repo.remote != remote.as_ref() {
                return false;
            }
            match repo.name.strip_prefix(prefix.as_ref()) {
                Some(base) => base.contains(query.as_ref()),
                None => false,
            }
        })
    }

    pub fn list_remote(&self, remote: impl AsRef<str> + Into<String>) -> Vec<&Repo> {
        self.borrow_repos()
            .iter()
            .filter(|repo| repo.name.starts_with(remote.as_ref()))
            .collect()
    }

    pub fn list(&self) -> Vec<&Repo> {
        self.borrow_repos().iter().collect()
    }

    pub fn update(&mut self, name: impl AsRef<str> + Into<String>, now: Epoch) {
        self.with_repos_mut(|repos| {
            if let Some(repo) = repos.iter_mut().find(|repo| repo.name == name.as_ref()) {
                repo.accessed += 1.0;
                repo.last_accessed = now;
            }
        });
    }

    pub fn remove(&mut self, name: impl AsRef<str> + Into<String>) {
        if let Some(idx) = self
            .borrow_repos()
            .iter()
            .position(|repo| repo.name == name.as_ref())
        {
            self.swap_remove(idx);
        }
    }

    pub fn remove_by_path(&mut self, path: impl AsRef<str> + Into<String>) {
        if let Some(idx) = self
            .borrow_repos()
            .iter()
            .position(|repo| repo.path == path.as_ref())
        {
            self.swap_remove(idx);
        }
    }

    pub fn swap_remove(&mut self, idx: usize) {
        self.with_repos_mut(|repos| repos.swap_remove(idx));
        self.with_dirty_mut(|d| *d = true);
    }

    pub fn dirty(&self) -> bool {
        *self.borrow_dirty()
    }

    pub fn sort(&mut self, now: Epoch) {
        self.with_repos_mut(|repos| {
            repos.sort_unstable_by(|repo1: &Repo, repo2: &Repo| {
                repo1.score(now).total_cmp(&repo2.score(now))
            })
        });
        self.with_dirty_mut(|d| *d = true);
    }

    fn serialize(repos: &[Repo<'_>]) -> Result<Vec<u8>> {
        (|| -> bincode::Result<_> {
            let buffer_size =
                bincode::serialized_size(&Self::VERSION)? + bincode::serialized_size(&repos)?;
            let mut buffer = Vec::with_capacity(buffer_size as usize);

            bincode::serialize_into(&mut buffer, &Self::VERSION)?;
            bincode::serialize_into(&mut buffer, &repos)?;

            Ok(buffer)
        })()
        .context("could not serialize database")
    }

    fn deserialize(bytes: &[u8]) -> Result<Vec<Repo>> {
        // Assume a maximum size for the database. This prevents
        // bincode from throwing strange errors when it encounters
        // invalid data.
        const MAX_SIZE: u64 = 32 << 10; // 32 MiB

        let deserializer = &mut bincode::options()
            .with_fixint_encoding()
            .with_limit(MAX_SIZE);

        let version_size = deserializer.serialized_size(&Self::VERSION).unwrap() as _;
        if bytes.len() < version_size {
            bail!("could not deserialize database: corrupted data");
        }
        let (bytes_version, bytes_repos) = bytes.split_at(version_size);
        let version = deserializer.deserialize(bytes_version)?;

        let repos = match version {
            Self::VERSION => deserializer
                .deserialize(bytes_repos)
                .context("could not deserialize repo data")?,
            version => bail!("unsupported version {version}, supports: {}", Self::VERSION),
        };

        Ok(repos)
    }
}
