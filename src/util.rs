use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::mem;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
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
// pub const MONTH: Epoch = 30 * DAY;

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

pub fn split_name<'a>(query: impl AsRef<str>) -> (String, String) {
    let items: Vec<_> = query.as_ref().split("/").collect();
    let items_len = items.len();
    let mut group_buffer: Vec<String> = Vec::with_capacity(items_len - 1);
    let mut base = "";
    for (idx, item) in items.iter().enumerate() {
        if idx == items_len - 1 {
            base = item;
        } else {
            group_buffer.push(item.to_string());
        }
    }
    (group_buffer.join("/"), base.to_string())
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
    pub fn build() -> Result<Fzf> {
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

const ERR_GIT_NOT_FOUND: &str = "could not find git, is it installed?";

pub struct Git(Command);

impl Git {
    pub fn new() -> Git {
        // TODO: support Windows
        let program = "git";
        let mut cmd = Command::new(program);
        cmd.stdout(Stdio::piped());
        Git(cmd)
    }

    pub fn with_path<S>(&mut self, path: S) -> &mut Self
    where
        S: AsRef<str>,
    {
        self.args(["-C", path.as_ref()]);
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.0.args(args);
        self
    }

    pub fn arg<S>(&mut self, arg: S) -> &mut Self
    where
        S: AsRef<OsStr>,
    {
        self.0.arg(arg);
        self
    }

    pub fn exec(&mut self) -> Result<String> {
        let args = self.0.get_args();
        let args: Vec<&OsStr> = args.collect();
        let mut strs: Vec<&str> = Vec::with_capacity(args.len() + 1);
        strs.push("git");
        for arg in &args {
            let str = match arg.to_str() {
                Some(s) => s,
                None => continue,
            };
            strs.push(str);
        }
        let cmd_str = strs.join(" ");
        _ = writeln!(
            io::stderr(),
            "{} {}",
            style("==>").cyan(),
            style(cmd_str).bold()
        );

        let mut child = match self.0.spawn() {
            Ok(child) => child,
            Err(e) if e.kind() == io::ErrorKind::NotFound => bail!(ERR_GIT_NOT_FOUND),
            Err(e) => return Err(e).context("could not launch git"),
        };

        let mut stdout = child.stdout.take().unwrap();
        let mut output = String::new();
        stdout
            .read_to_string(&mut output)
            .context("failed to read from git")?;

        let status = child.wait().context("wait failed on git")?;
        match status.code() {
            Some(0) => Ok(output),
            _ => bail!(SilentExit { code: 101 }),
        }
    }
}

pub struct EmptyDir {
    path: PathBuf,
    subs: Vec<EmptyDir>,

    empty: bool,
    keep: bool,
}

impl EmptyDir {
    pub fn scan<S, E>(path: S, exclude: &Vec<E>) -> Result<EmptyDir>
    where
        S: AsRef<str>,
        E: AsRef<OsStr>,
    {
        let mut exclude_set: HashSet<&OsStr> = HashSet::with_capacity(exclude.len());
        for s in exclude {
            exclude_set.insert(s.as_ref());
        }

        let path = PathBuf::from_str(path.as_ref()).context("invalid scan path")?;
        let mut root = EmptyDir {
            path,
            subs: vec![],
            empty: false,
            keep: false,
        };
        root.walk(&exclude_set)?;
        root.mark();
        Ok(root)
    }

    fn walk(&mut self, exclude: &HashSet<&OsStr>) -> Result<()> {
        let subs = fs::read_dir(&self.path).with_context(|| {
            format!(
                "could not read directory {}",
                PathBuf::from(&self.path).display()
            )
        })?;
        let mut keep = false;
        let mut sub_dirs = vec![];
        for sub in subs {
            let sub = sub.context("could not read sub directory")?;
            let meta = sub
                .metadata()
                .context("could not read meta data for sub directory")?;
            if meta.is_file() {
                keep = true;
                continue;
            }
            let sub_path = self.path.join(sub.file_name());
            if let Some(_) = exclude.get(sub_path.as_os_str()) {
                keep = true;
                continue;
            }
            let mut sub_dir = EmptyDir {
                path: sub_path,
                subs: vec![],
                empty: false,
                keep: false,
            };
            sub_dir.walk(exclude)?;
            sub_dirs.push(sub_dir);
        }
        if sub_dirs.is_empty() {
            self.empty = true;
            return Ok(());
        }
        self.keep = keep;
        Ok(())
    }

    fn mark(&mut self) {
        if self.subs.is_empty() {
            return;
        }
        let mut empty = true;
        for sub in &mut self.subs {
            sub.mark();
            if !sub.empty {
                empty = false;
            }
        }
        if !self.keep {
            self.empty = empty;
        }
    }
}
