use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::mem;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use anyhow::anyhow;

use anyhow::{Context, Result};

use crate::db::Epoch;

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
