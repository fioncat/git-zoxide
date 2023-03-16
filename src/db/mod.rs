mod repo;

use std::{fs, io, path::PathBuf};

use anyhow::{bail, Context, Result};
use bincode::Options;

pub use crate::db::repo::{Epoch, Repo};
use crate::{config, util};

pub struct Database {
    path: PathBuf,
    pub repos: Vec<Repo>,
}

impl Database {
    const VERSION: u32 = 1;

    pub fn open() -> Result<Database> {
        let data_dir = config::Config::get_data_dir()?;
        let path = data_dir.join("database");

        match fs::read(&path) {
            Ok(bytes) => Ok(Database {
                path,
                repos: Self::deserialize(&bytes)?,
            }),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                fs::create_dir_all(&data_dir).with_context(|| {
                    format!("unable to create data directory: {}", data_dir.display())
                })?;
                Ok(Database {
                    path,
                    repos: vec![],
                })
            }
            Err(err) => bail!("failed to read db: {err}"),
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let bytes = Self::serialize(&self.repos)?;
        if let Err(err) = util::write(&self.path, bytes) {
            bail!("failed to write database: {err}");
        }

        Ok(())
    }

    pub fn get<R, N>(&self, remote: R, name: N) -> Option<usize>
    where
        R: AsRef<str>,
        N: AsRef<str>,
    {
        self.repos
            .iter()
            .position(|repo| repo.remote == remote.as_ref() && repo.name == name.as_ref())
    }

    pub fn add<R, N, P>(&mut self, remote: R, name: N, path: P) -> usize
    where
        R: AsRef<str>,
        N: AsRef<str>,
        P: AsRef<str>,
    {
        self.repos.push(Repo {
            remote: remote.as_ref().to_string(),
            name: name.as_ref().to_string(),
            path: path.as_ref().to_string(),
            last_accessed: 0,
            accessed: 0.0,
        });
        self.repos.len() - 1
    }

    pub fn update(&mut self, idx: usize, now: Epoch) {
        let mut repo = &mut self.repos[idx];
        repo.last_accessed = now;
        repo.accessed += 1.0;
    }

    pub fn sort(&mut self, now: Epoch) {
        self.repos.sort_unstable_by(|repo1: &Repo, repo2: &Repo| {
            repo1.score(now).total_cmp(&repo2.score(now))
        })
    }

    fn serialize(repos: &[Repo]) -> Result<Vec<u8>> {
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
