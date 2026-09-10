use crate::ErrorCode;
use crate::{Error, Result, revision};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};
pub const MAX_NOTE_BYTES: usize = 16 * 1024 * 1024;

// Only ordinary Unix mode-bit files are supported for replacement. Refuse
// security metadata we cannot preserve instead of broadening effective access.
fn plain_security(file: &File) -> Result<()> {
    let mut attributes = [0u8; 65536];
    let count = rustix::fs::flistxattr(file, &mut attributes[..])
        .map_err(|e| Error::new(ErrorCode::Unsupported, e))?;
    if count != 0 || file.metadata()?.mode() & 0o7000 != 0 {
        return Err(Error::new(
            ErrorCode::Unsupported,
            "ACLs, extended attributes and special mode bits are not supported for replacement",
        ));
    }
    Ok(())
}

/// Persist each newly created directory entry before a note can be committed.
pub fn ensure_directory(path: &Path) -> Result<()> {
    check_chain(path)?;
    let mut missing = Vec::new();
    let mut current = path;
    while !current.exists() {
        missing.push(current.to_path_buf());
        current = current
            .parent()
            .ok_or_else(|| Error::new(ErrorCode::Path, "no existing ancestor"))?;
    }
    for directory in missing.iter().rev() {
        match fs::create_dir(directory) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        check_chain(directory)?;
        File::open(directory.parent().unwrap())?.sync_all()?;
    }
    check_chain(path)?;
    if !path.is_dir() {
        return Err(Error::new(ErrorCode::Path, "expected directory"));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct Commit {
    pub file_committed: bool,
    pub durability_confirmed: bool,
    pub path: String,
    pub revision: Option<crate::Revision>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct LibraryRelativePath(String);
impl LibraryRelativePath {
    pub fn parse(path: &str) -> Result<Self> {
        relative(path)
    }
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
pub fn relative(s: &str) -> Result<LibraryRelativePath> {
    if s.is_empty()
        || s.contains(['\\', ':', '\0'])
        || !s.ends_with(".md")
        || s.split('/').any(|c| {
            c.is_empty()
                || c == "."
                || c == ".."
                || c.ends_with([' ', '.'])
                || c.chars().any(|x| x.is_control() || "<>\"|?*".contains(x))
                || {
                    let stem = c.split('.').next().unwrap_or("").to_ascii_uppercase();
                    [
                        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
                        "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
                        "LPT7", "LPT8", "LPT9",
                    ]
                    .contains(&stem.as_str())
                }
        })
        || Path::new(s)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(Error::new(
            ErrorCode::Path,
            "expected safe relative lowercase .md path",
        ));
    }
    Ok(LibraryRelativePath(s.into()))
}
/// Validate every existing component without following links. Not hostile-ancestor CAS.
pub fn check_chain(path: &Path) -> Result<()> {
    let mut p = PathBuf::new();
    for c in path.components() {
        if matches!(c, Component::ParentDir) {
            return Err(Error::new(ErrorCode::Path, "parent traversal"));
        }
        p.push(c);
        match fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(Error::new(ErrorCode::Path, "symlink component"));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
pub fn safe_path(root: &Path, s: &str) -> Result<PathBuf> {
    let p = root.join(relative(s)?.as_path());
    check_chain(&p)?;
    Ok(p)
}
pub fn read(path: &Path) -> Result<Vec<u8>> {
    check_chain(path)?;
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(
            rustix::fs::OFlags::NOFOLLOW.bits() as i32 | rustix::fs::OFlags::NONBLOCK.bits() as i32,
        )
        .open(path)?;
    let m = f.metadata()?;
    if !m.is_file() {
        return Err(Error::new(ErrorCode::Unsupported, "not a regular file"));
    }
    if m.len() > 16 * 1024 * 1024 {
        return Err(Error::new(ErrorCode::Unsupported, "note exceeds 16 MiB"));
    }
    let mut b = Vec::new();
    f.take(16 * 1024 * 1024 + 1).read_to_end(&mut b)?;
    if b.len() > 16 * 1024 * 1024 {
        return Err(Error::new(ErrorCode::Unsupported, "note exceeds 16 MiB"));
    }
    Ok(b)
}
pub fn writable(path: &Path) -> Result<fs::Metadata> {
    check_chain(path)?;
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() || m.nlink() != 1 {
        return Err(Error::new(
            ErrorCode::Unsupported,
            "mutation requires regular singly-linked file",
        ));
    }
    if m.mode() & 0o222 == 0 {
        return Err(Error::new(ErrorCode::Permission, "read-only note"));
    }
    Ok(m)
}
pub fn precondition(path: &Path, expected: &crate::Revision) -> Result<fs::Metadata> {
    let m = writable(path)?;
    if revision(&read(path)?) != *expected {
        return Err(Error::new(ErrorCode::Conflict, "note revision changed"));
    }
    Ok(m)
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    BeforeWrite,
    BeforeSync,
    BeforeReplace,
    AfterReplace,
}
pub fn save(path: &Path, bytes: &[u8], expected: Option<&crate::Revision>) -> Result<Commit> {
    save_observed_with_security(path, bytes, expected, true, |_| Ok(()))
}
/// Application-owned configuration is not user data. On macOS, inherited ACLs
/// and extended attributes are common even under an otherwise ordinary home
/// directory; refusing to replace the config would prevent selecting a library.
pub fn save_config(
    path: &Path,
    bytes: &[u8],
    expected: Option<&crate::Revision>,
) -> Result<Commit> {
    save_observed_with_security(path, bytes, expected, false, |_| Ok(()))
}
/// Observer is for deterministic commit-boundary tests; errors after replace are committed outcomes.
pub fn save_observed(
    path: &Path,
    bytes: &[u8],
    expected: Option<&crate::Revision>,
    observe: impl FnMut(Stage) -> Result<()>,
) -> Result<Commit> {
    save_observed_with_security(path, bytes, expected, true, observe)
}
fn save_observed_with_security(
    path: &Path,
    bytes: &[u8],
    expected: Option<&crate::Revision>,
    check_security: bool,
    mut observe: impl FnMut(Stage) -> Result<()>,
) -> Result<Commit> {
    if bytes.len() > MAX_NOTE_BYTES {
        return Err(Error::new(ErrorCode::Unsupported, "note exceeds 16 MiB"));
    }
    check_chain(path)?;
    let original = expected.map(|r| precondition(path, r)).transpose()?;
    let mode = original
        .as_ref()
        .map_or_else(|| fs::Permissions::from_mode(0o600), |m| m.permissions());
    if check_security && original.is_some() {
        let old = OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(path)?;
        plain_security(&old)?;
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(ErrorCode::Path, "no parent"))?;
    check_chain(parent)?;
    ensure_directory(parent)?;
    check_chain(parent)?;
    let mut temp = tempfile::Builder::new()
        .prefix(".foglio-stage-")
        .suffix(".tmp")
        .tempfile_in(parent)?;
    if check_security {
        plain_security(temp.as_file())?;
    }
    if let Some(old) = &original {
        let staged = temp.as_file().metadata()?;
        if old.uid() != staged.uid() || old.gid() != staged.gid() {
            return Err(Error::new(
                ErrorCode::Unsupported,
                "replacement would change note owner or group",
            ));
        }
    }
    observe(Stage::BeforeWrite)?;
    temp.write_all(bytes)?;
    observe(Stage::BeforeSync)?;
    temp.as_file().set_permissions(mode)?;
    temp.as_file().sync_all()?;
    observe(Stage::BeforeReplace)?;
    check_chain(path)?;
    if let Some(r) = expected {
        let now = precondition(path, r)?;
        let old = original.as_ref().unwrap();
        if (now.dev(), now.ino(), now.mode(), now.uid(), now.gid())
            != (old.dev(), old.ino(), old.mode(), old.uid(), old.gid())
        {
            return Err(Error::new(
                ErrorCode::Conflict,
                "note identity or permissions changed",
            ));
        }
        let current = OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(path)?;
        if check_security {
            plain_security(&current)?;
        }
        temp.persist(path).map_err(|e| Error::from(e.error))?;
    } else {
        // Require the Linux atomic no-replace primitive; no link/unlink fallback.
        rename_no_replace(temp.path(), path)?;
    }
    let durability_confirmed = observe(Stage::AfterReplace).is_ok()
        && File::open(parent).and_then(|f| f.sync_all()).is_ok();
    Ok(Commit {
        file_committed: true,
        durability_confirmed,
        path: path.to_string_lossy().into(),
        revision: Some(revision(bytes)),
    })
}
fn rename_no_replace(from: &Path, to: &Path) -> Result<()> {
    use rustix::{
        fs::{CWD, RenameFlags, renameat_with},
        io::Errno,
    };
    renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).map_err(|error| {
        let code = match error {
            Errno::EXIST => ErrorCode::Exists,
            Errno::XDEV | Errno::NOSYS | Errno::INVAL | Errno::OPNOTSUPP => ErrorCode::Unsupported,
            _ => ErrorCode::Io,
        };
        Error::new(code, error)
    })
}
pub fn move_file(from: &Path, to: &Path, expected: &crate::Revision) -> Result<Commit> {
    precondition(from, expected)?;
    check_chain(to)?;
    let parent = to
        .parent()
        .ok_or_else(|| Error::new(ErrorCode::Path, "no parent"))?;
    ensure_directory(parent)?;
    check_chain(to)?;
    precondition(from, expected)?;
    rename_no_replace(from, to)?;
    let durable = File::open(parent).and_then(|f| f.sync_all()).is_ok()
        && File::open(from.parent().unwrap())
            .and_then(|f| f.sync_all())
            .is_ok();
    Ok(Commit {
        file_committed: true,
        durability_confirmed: durable,
        path: to.to_string_lossy().into(),
        revision: Some(expected.clone()),
    })
}
pub fn delete_file(path: &Path, expected: &crate::Revision) -> Result<Commit> {
    precondition(path, expected)?;
    fs::remove_file(path)?;
    let durable = File::open(path.parent().unwrap())
        .and_then(|f| f.sync_all())
        .is_ok();
    Ok(Commit {
        file_committed: true,
        durability_confirmed: durable,
        path: path.to_string_lossy().into(),
        revision: None,
    })
}
/// Explicit unlock on drop also releases locks inherited by a concurrent fork
/// before that child reaches exec. Closing only our fd can leave its copy locked.
#[derive(Debug)]
pub struct LibraryLock(File, Option<crate::coordination::Permit>);
impl LibraryLock {
    pub(crate) fn with_local(mut self, permit: crate::coordination::Permit) -> Self {
        self.1 = Some(permit);
        self
    }
}
impl Drop for LibraryLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}
pub fn lock(path: &Path) -> Result<LibraryLock> {
    check_chain(path)?;
    ensure_directory(path.parent().unwrap())?;
    check_chain(path)?;
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)?;
    if !f.metadata()?.is_file() || f.metadata()?.nlink() != 1 {
        return Err(Error::new(ErrorCode::Unsupported, "hard-linked lock"));
    }
    fs2::FileExt::try_lock_exclusive(&f).map_err(|_| {
        Error::new(
            ErrorCode::Busy,
            "library is locked by another Foglio process",
        )
    })?;
    Ok(LibraryLock(f, None))
}
