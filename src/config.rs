use anyhow::bail;
use anyhow::{Context, Result};
use console::style;
use serde::Deserialize;

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::str::FromStr;

use crate::util::{self, Shell};

#[derive(Deserialize, Debug)]
pub struct Config {
    pub workspace: String,

    #[serde(default = "empty_vec")]
    pub remotes: Vec<Remote>,

    #[serde(default = "empty_map")]
    pub keyword_map: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
pub struct Remote {
    pub name: String,
    pub user: Option<User>,
    pub clone: Option<Clone>,
    pub api: Option<API>,

    #[serde(default = "empty_vec")]
    pub on_create: Vec<Step>,
}

#[derive(Deserialize, Debug)]
pub struct Step {
    pub name: String,
    pub run: Option<String>,
    pub file: Option<String>,
    pub copy: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub name: String,
    pub email: String,
}

#[derive(Deserialize, Debug)]
pub struct Clone {
    pub domain: String,

    #[serde(default = "default_bool")]
    pub use_ssh: bool,

    #[serde(default = "empty_string")]
    pub ssh_groups: String,
}

#[derive(Deserialize, Debug)]
pub struct API {
    pub provider: Provider,

    #[serde(default = "empty_string")]
    pub token: String,

    #[serde(default = "empty_string")]
    pub url: String,
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

fn empty_map<K, V>() -> HashMap<K, V> {
    HashMap::new()
}

fn default_bool() -> bool {
    false
}

fn default_config() -> Config {
    Config {
        workspace: String::from("${HOME}/dev"),
        keyword_map: empty_map(),
        remotes: vec![],
    }
}

impl Config {
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
        let path = Self::get_path()?;
        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(default_config()),
            Err(err) => return Err(err).context("could not read config file"),
        };
        match serde_yaml::from_reader(file) {
            Ok(config) => Ok(config),
            Err(err) => return Err(err).context("could not parse config yaml"),
        }
    }

    pub fn parse() -> Result<Config> {
        let mut config = Self::read_config()?;
        if let Err(err) = config.normalize() {
            return Err(err).context("unable to validate config");
        };
        Ok(config)
    }

    fn normalize(&mut self) -> Result<()> {
        self.workspace = util::expand_env(&self.workspace)?;
        let mut remote_set: HashSet<&String> = HashSet::with_capacity(self.remotes.len());
        for remote in &mut self.remotes {
            if let Some(_) = remote_set.get(&remote.name) {
                bail!("remote {} is duplicate in your config", remote.name)
            }
            remote_set.insert(&remote.name);

            if let Some(api) = &mut remote.api {
                api.token = util::expand_env(&api.token)?;
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
            None => bail!("could not find remote {}", style(name).yellow()),
        }
    }
}

impl Step {
    pub fn exec(&self, path: &PathBuf, env: &Vec<(&str, &str)>) -> Result<()> {
        if let Some(run) = self.run.as_ref() {
            let script = run.replace("\n", ";");

            util::print_operation(format!("exec {} ...", style(&self.name).yellow()));
            let mut cmd = Shell::bash(&script);
            for (key, val) in env {
                cmd.env(key, val);
            }
            cmd.with_path(path);
            cmd.exec()?;
            return Ok(());
        }

        util::print_operation(format!("create {} ...", style(&self.name).yellow()));

        let dst_name = PathBuf::from_str(&self.name)?;
        let dst_path = path.join(dst_name);
        let dst_dir = dst_path.parent().unwrap();

        match fs::read_dir(&dst_dir) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                fs::create_dir_all(&dst_dir)
                    .with_context(|| format!("could not create dir {}", dst_dir.display()))?;
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("could not read dir {}", dst_dir.display()));
            }
            _ => {}
        }

        if let Some(copy) = self.copy.as_ref() {
            let src_path = util::expand_env(copy)?;
            let src_path = PathBuf::from_str(&src_path)?;
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "could not copy from {} to {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
            return Ok(());
        }

        let content = match self.file.as_ref() {
            Some(s) => s,
            None => "",
        };
        let content = content.replace("\\t", "\t");

        fs::write(&dst_path, content)
            .with_context(|| format!("could not write {}", dst_path.display()))?;
        Ok(())
    }
}
