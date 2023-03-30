mod repo;

use console::style;
use std::{collections::HashMap, fs, io, path::PathBuf};

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
            Err(err) => Err(err).context("could not read database file"),
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let bytes = Self::serialize(&self.repos)?;
        if let Err(err) = util::write(&self.path, bytes) {
            return Err(err).context("could not write database file");
        }

        Ok(())
    }

    pub fn current(&self, workspace: impl AsRef<str>) -> Result<&Repo> {
        let current_dir = util::current_dir()?;

        for repo in &self.repos {
            let path = repo.path(workspace.as_ref())?;
            if current_dir.eq(&path) {
                return Ok(repo);
            }
        }

        bail!("current path does not bound to any repository")
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

    pub fn get_by_path<S>(&self, path: S) -> Option<usize>
    where
        S: AsRef<str>,
    {
        self.repos
            .iter()
            .position(|repo| repo.path != "" && repo.path == path.as_ref())
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

    pub fn must_get<R, N>(&self, remote: R, name: N) -> Result<usize>
    where
        R: AsRef<str>,
        N: AsRef<str>,
    {
        match self.repos.iter().position(|repo| {
            repo.remote.as_str() == remote.as_ref() && repo.name.as_str() == name.as_ref()
        }) {
            Some(idx) => Ok(idx),
            None => bail!(
                "could not find repository {}:{}",
                style(remote.as_ref()).yellow(),
                style(name.as_ref()).yellow()
            ),
        }
    }

    pub fn list_paths(&self, workspace: &String) -> Result<Vec<PathBuf>> {
        let mut paths: Vec<PathBuf> = Vec::with_capacity(self.repos.len());
        for repo in &self.repos {
            let path = repo.path(workspace)?;
            paths.push(path);
        }
        Ok(paths)
    }

    pub fn match_keyword<R, K>(&self, remote: R, keyword: K) -> Result<usize>
    where
        R: AsRef<str>,
        K: AsRef<str>,
    {
        let (group, base) = util::split_name(keyword.as_ref());
        let opt = self.repos.iter().position(|repo| {
            if remote.as_ref() != "" && repo.remote != remote.as_ref() {
                return false;
            }
            let (repo_group, repo_base) = util::split_name(&repo.name);
            if group == "" {
                return repo_base.contains(&base);
            }

            repo_group == group && repo_base.contains(&base)
        });
        match opt {
            Some(idx) => Ok(idx),
            None => bail!(
                "could not find repository matches {}",
                style(keyword.as_ref()).yellow()
            ),
        }
    }

    pub fn update(&mut self, idx: usize, now: Epoch) {
        let mut repo = &mut self.repos[idx];
        repo.last_accessed = now;
        repo.accessed += 1.0;
    }

    pub fn sort(&mut self, now: Epoch) {
        self.repos.sort_unstable_by(|repo1: &Repo, repo2: &Repo| {
            repo2.score(now).total_cmp(&repo1.score(now))
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

pub struct Keywords {
    path: PathBuf,
    pub data: HashMap<String, Epoch>,
}

impl Keywords {
    const VERSION: u32 = 1;

    pub fn open(now: Epoch) -> Result<Keywords> {
        let data_dir = config::Config::get_data_dir()?;
        let path = data_dir.join("keywords");

        match fs::read(&path) {
            Ok(bytes) => Ok(Keywords {
                path,
                data: Self::deserialize(&bytes, now)?,
            }),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                fs::create_dir_all(&data_dir).with_context(|| {
                    format!("unable to create data directory: {}", data_dir.display())
                })?;
                Ok(Keywords {
                    path,
                    data: HashMap::new(),
                })
            }
            Err(err) => Err(err).context("could not open keywords file"),
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let bytes = Self::serialize(&self.data)?;
        if let Err(err) = util::write(&self.path, bytes) {
            return Err(err).context("could not write keywords file");
        }

        Ok(())
    }

    pub fn list(&self) -> Vec<&str> {
        let mut vec: Vec<&str> = self.data.iter().map(|(key, _)| key.as_str()).collect();
        vec.sort_by(|s1, s2| s1.cmp(s2));
        vec
    }

    pub fn add(&mut self, keyword: &str, now: Epoch) {
        self.data.insert(keyword.to_string(), now + util::DAY);
    }

    fn serialize(data: &HashMap<String, Epoch>) -> Result<Vec<u8>> {
        (|| -> bincode::Result<_> {
            let buffer_size =
                bincode::serialized_size(&Self::VERSION)? + bincode::serialized_size(&data)?;
            let mut buffer = Vec::with_capacity(buffer_size as usize);

            bincode::serialize_into(&mut buffer, &Self::VERSION)?;
            bincode::serialize_into(&mut buffer, &data)?;

            Ok(buffer)
        })()
        .context("could not serialize database")
    }

    fn deserialize(bytes: &[u8], now: Epoch) -> Result<HashMap<String, Epoch>> {
        const MAX_SIZE: u64 = 32 << 10; // 32 MiB

        let deserializer = &mut bincode::options()
            .with_fixint_encoding()
            .with_limit(MAX_SIZE);

        let version_size = deserializer.serialized_size(&Self::VERSION).unwrap() as _;
        if bytes.len() < version_size {
            bail!("could not deserialize database: corrupted data");
        }
        let (bytes_version, bytes_data) = bytes.split_at(version_size);
        let version = deserializer.deserialize(bytes_version)?;

        let data: HashMap<String, Epoch> = match version {
            Self::VERSION => deserializer
                .deserialize(bytes_data)
                .context("could not deserialize repo data")?,
            version => bail!("unsupported version {version}, supports: {}", Self::VERSION),
        };

        let data: HashMap<String, Epoch> = data
            .iter()
            .filter(|(_, expire)| expire >= &&now)
            .map(|(key, expire)| (key.to_string(), *expire))
            .collect();
        Ok(data)
    }
}
