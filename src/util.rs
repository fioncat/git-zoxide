use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::mem;
use std::ops::Index;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::SystemTime;

use anyhow::{bail, Context, Result};

use crate::db::Epoch;
use crate::errors::SilentExit;

use console::{style, Term};
use dialoguer::{theme::ColorfulTheme, Confirm};

pub const SECOND: Epoch = 1;
pub const MINUTE: Epoch = 60 * SECOND;
pub const HOUR: Epoch = 60 * MINUTE;
pub const DAY: Epoch = 24 * HOUR;
pub const WEEK: Epoch = 7 * DAY;
pub const MONTH: Epoch = 30 * DAY;

/// Similar to [`fs::write`], but atomic (best effort on Windows).
pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    let contents = contents.as_ref();
    let dir = path.parent().unwrap();

    // Create a tmpfile.
    let (mut tmp_file, tmp_path) = tmpfile(dir)?;
    let result = (|| {
        // Write to the tmpfile.
        _ = tmp_file.set_len(contents.len() as u64);
        tmp_file
            .write_all(contents)
            .with_context(|| format!("could not write to file: {}", tmp_path.display()))?;

        // Set the owner of the tmpfile (UNIX only).
        #[cfg(unix)]
        if let Ok(metadata) = path.metadata() {
            use std::os::unix::fs::MetadataExt;
            use std::os::unix::io::AsRawFd;

            use nix::unistd::{self, Gid, Uid};

            let uid = Uid::from_raw(metadata.uid());
            let gid = Gid::from_raw(metadata.gid());
            _ = unistd::fchown(tmp_file.as_raw_fd(), Some(uid), Some(gid));
        }

        // Close and rename the tmpfile.
        mem::drop(tmp_file);
        rename(&tmp_path, path)
    })();
    // In case of an error, delete the tmpfile.
    if result.is_err() {
        _ = fs::remove_file(&tmp_path);
    }
    result
}

/// Atomically create a tmpfile in the given directory.
fn tmpfile(dir: impl AsRef<Path>) -> Result<(File, PathBuf)> {
    const MAX_ATTEMPTS: usize = 5;
    const TMP_NAME_LEN: usize = 16;
    let dir = dir.as_ref();

    let mut attempts = 0;
    loop {
        attempts += 1;

        // Generate a random name for the tmpfile.
        let mut name = String::with_capacity(TMP_NAME_LEN);
        name.push_str("tmp_");
        while name.len() < TMP_NAME_LEN {
            name.push(fastrand::alphanumeric());
        }
        let path = dir.join(name);

        // Atomically create the tmpfile.
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => break Ok((file, path)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && attempts < MAX_ATTEMPTS => {}
            Err(e) => {
                break Err(e).with_context(|| format!("could not create file: {}", path.display()));
            }
        }
    }
}

/// Similar to [`fs::rename`], but with retries on Windows.
fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();

    const MAX_ATTEMPTS: usize = if cfg!(windows) { 5 } else { 1 };
    let mut attempts = 0;

    loop {
        match fs::rename(from, to) {
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied && attempts < MAX_ATTEMPTS => {
                attempts += 1
            }
            result => {
                break result.with_context(|| {
                    format!(
                        "could not rename file: {} -> {}",
                        from.display(),
                        to.display()
                    )
                });
            }
        }
    }
}

pub fn split_query(query: impl AsRef<str> + Into<String>) -> (String, String) {
    let items: Vec<_> = query.as_ref().split("/").collect();
    let items_len = items.len();
    let mut group_buffer: Vec<String> = Vec::with_capacity(items_len - 1);
    let mut base = String::new();
    for (idx, item) in items.iter().enumerate() {
        if idx == items_len - 1 {
            base = item.to_string();
        } else {
            group_buffer.push(item.to_string());
        }
    }
    (group_buffer.join("/"), base)
}

pub fn current_time() -> Result<Epoch> {
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("system clock set to invalid time")?
        .as_secs();

    Ok(current_time)
}

pub fn confirm(msg: impl AsRef<str> + Into<String>) -> Result<()> {
    let result = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(msg)
        .interact_on(&Term::stderr());
    match result {
        Ok(ok) => {
            if !ok {
                bail!(SilentExit { code: 60 })
            }
            Ok(())
        }
        Err(err) => Err(err).context("could not do confirm prompt"),
    }
}

const ERR_FZF_NOT_FOUND: &str = "could not find fzf, is it installed?";

pub struct Fzf(Child);

impl Fzf {
    pub fn create() -> Result<Fzf> {
        // TODO: support Windows
        let program = "fzf";
        let mut cmd = Command::new(program);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());

        match cmd.spawn() {
            Ok(child) => Ok(Fzf(child)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => bail!(ERR_FZF_NOT_FOUND),
            Err(err) => Err(err).context("could not launch fzf"),
        }
    }

    pub fn query<S>(&mut self, keys: &Vec<S>) -> Result<usize>
    where
        S: AsRef<str>,
    {
        let mut input = String::with_capacity(keys.len());
        for key in keys {
            input.push_str(key.as_ref());
            input.push_str("\n");
        }

        let handle = self.0.stdin.as_mut().unwrap();
        if let Err(err) = write!(handle, "{}", input) {
            return Err(err).context("could not write to fzf");
        }

        mem::drop(self.0.stdin.take());

        let mut stdout = self.0.stdout.take().unwrap();
        let mut output = String::new();
        stdout
            .read_to_string(&mut output)
            .context("failed to read from fzf")?;
        let output = output.trim();
        let status = self.0.wait().context("wait failed on fzf")?;
        match status.code() {
            Some(0) => match keys.iter().position(|s| s.as_ref() == output) {
                Some(idx) => Ok(idx),
                None => bail!("could not find key {}", output),
            },
            Some(1) => bail!("no match found"),
            Some(2) => bail!("fzf returned an error"),
            Some(130) => bail!(SilentExit { code: 130 }),
            Some(128..=254) | None => bail!("fzf was terminated"),
            _ => bail!("fzf returned an unknown error"),
        }
    }
}
