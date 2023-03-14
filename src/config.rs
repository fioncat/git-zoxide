use anyhow::bail;
use anyhow::{Context, Result};
use serde::Deserialize;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
pub struct Config {
    workspace: String,

    #[serde(default = "empty_vec")]
    remotes: Vec<Remote>,
}

#[derive(Deserialize, Debug)]
pub struct Remote {
    name: String,
    user: Option<User>,
    clone: Option<Clone>,
    api: Option<API>,
}

#[derive(Deserialize, Debug)]
pub struct User {
    name: String,
    email: String,
}

#[derive(Deserialize, Debug)]
pub struct Clone {
    domain: String,

    #[serde(default = "default_bool")]
    use_ssh: bool,

    #[serde(default = "empty_string")]
    ssh_groups: String,
}

#[derive(Deserialize, Debug)]
pub struct API {
    provider: Provider,

    #[serde(default = "empty_string")]
    token: String,
}

#[derive(Deserialize, Debug)]
pub enum Provider {
    #[serde(rename = "github")]
    Github,
    #[serde(rename = "gitlab")]
    Gitlab,
}

fn empty_string() -> String {
    String::new()
}

fn empty_vec<T>() -> Vec<T> {
    vec![]
}

fn default_bool() -> bool {
    false
}

fn default_config() -> Config {
    Config {
        workspace: String::from("${HOME}/dev"),
        remotes: vec![],
    }
}

impl Config {
    fn normalize(&mut self) -> Result<()> {
        self.workspace = match shellexpand::full(&self.workspace) {
            Ok(path) => path.to_string(),
            Err(err) => bail!("failed to expand workspace env: {err}"),
        };
        let mut remote_set: HashMap<&String, ()> = HashMap::new();
        for remote in &mut self.remotes {
            if let Some(_) = remote_set.get(&remote.name) {
                bail!("remote {} is duplicate", remote.name)
            }
            remote_set.insert(&remote.name, ());

            if let Some(api) = &mut remote.api {
                api.token = match shellexpand::env(&api.token) {
                    Ok(token) => token.to_string(),
                    Err(err) => bail!(
                        "failed to expand api token to remote {}: {err}",
                        remote.name
                    ),
                };
            };
        }
        Ok(())
    }

    pub fn get_remote<'a>(&'a self, name: &str) -> Option<&'a Remote> {
        self.remotes.iter().find(|remote| remote.name == name)
    }

    pub fn must_get_remote<'a>(&'a self, name: &str) -> Result<&'a Remote> {
        match self.get_remote(name) {
            Some(remote) => Ok(remote),
            None => bail!("could not find remote {name}"),
        }
    }
}

pub fn get_path() -> Result<PathBuf> {
    let path = match env::var_os("_GZ_CONFIG_PATH") {
        Some(path) => PathBuf::from(path),
        None => dirs::config_dir()
            .context("could not find config directory, please set _GZ_CONFIG_PATH")?
            .join("git-zoxide")
            .join("config.yaml"),
    };
    Ok(path)
}

pub fn get_data_dir() -> Result<PathBuf> {
    let path = match env::var_os("_GZ_DATA_PATH") {
        Some(path) => PathBuf::from(path),
        None => dirs::data_local_dir()
            .context("could not find data directory, please set _GZ_DATA_PATH manually")?
            .join("git-zoxide"),
    };
    Ok(path)
}

fn read_config() -> Result<Config> {
    let path = get_path()?;
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(default_config()),
        Err(err) => bail!("failed to read config file: {err}"),
    };
    match serde_yaml::from_reader(file) {
        Ok(config) => Ok(config),
        Err(err) => bail!("failed to parse config yaml: {err}"),
    }
}

pub fn parse() -> Result<Config> {
    let mut config = read_config()?;
    if let Err(err) = config.normalize() {
        bail!("failed to validate config: {err}")
    };
    Ok(config)
}
