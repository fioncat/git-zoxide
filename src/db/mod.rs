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
        let data_dir = config::get_data_dir()?;
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

    pub fn insert(
        &mut self,
        name: impl AsRef<str> + Into<String>,
        path: impl AsRef<str> + Into<String>,
        now: Epoch,
    ) {
        self.with_repos_mut(|repos| {
            if let None = repos.iter().find(|repo| repo.name == name.as_ref()) {
                repos.push(Repo {
                    name: name.into().into(),
                    path: path.into().into(),
                    last_accessed: now,
                    accessed: 1,
                });
            }
        });
        self.with_dirty_mut(|d| *d = true);
    }

    pub fn get(&self, name: impl AsRef<str> + Into<String>) -> Option<&Repo> {
        self.borrow_repos()
            .iter()
            .find(|repo| repo.name == name.as_ref())
    }

    pub fn get_by_path(&self, path: impl AsRef<str> + Into<String>) -> Option<&Repo> {
        self.borrow_repos()
            .iter()
            .find(|repo| repo.path == path.as_ref())
    }

    pub fn update(&mut self, name: impl AsRef<str> + Into<String>, now: Epoch) {
        self.with_repos_mut(|repos| {
            if let Some(repo) = repos.iter_mut().find(|repo| repo.name == name.as_ref()) {
                repo.accessed += 1;
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
