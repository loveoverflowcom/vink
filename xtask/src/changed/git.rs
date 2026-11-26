//! Pinned, read-only Git inputs. Every query runs without optional locks, fsmonitor or lazy
//! fetch; nothing here writes the index, objects or refs.
use sha2::{Digest, Sha256};
use std::{fmt::Write, path::Path, process::Command};

/// The executor always uses its compiled checkout. A hook or exported alternate index
/// must not supply a different set of Git inputs for a coverage claim about that tree.
fn guard() -> Result<(), String> {
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
    ] {
        if std::env::var_os(key).is_some() {
            return Err(format!(
                "{key} overrides Git inputs; rerun from the intended checkout without that override"
            ));
        }
    }
    Ok(())
}

pub fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    guard()?;
    let out = Command::new("git")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(["--no-optional-locks", "-c", "core.fsmonitor=false"])
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("read Git inputs: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args[0],
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

fn text(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes)
        .map(|s| s.trim().to_string())
        .map_err(|e| e.to_string())
}

/// Resolves a revision to a commit without accepting options or ranges.
pub fn commit(root: &Path, rev: &str) -> Result<String, String> {
    guard()?;
    if rev.is_empty() || rev.starts_with('-') || rev.contains("..") {
        return Err(format!(
            "invalid revision `{rev}`: expected one commit reference"
        ));
    }
    git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &format!("{rev}^{{commit}}"),
        ],
    )
    .and_then(text)
    .ok()
    .filter(|sha| !sha.is_empty())
    .ok_or_else(|| {
        format!(
            "unknown revision `{rev}` (xtask never fetches; fetch it or pass an existing --base)"
        )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Committed,
    Index,
    Worktree,
    Untracked,
}

impl Layer {
    pub fn name(self) -> &'static str {
        match self {
            Self::Committed => "committed",
            Self::Index => "staged",
            Self::Worktree => "unstaged",
            Self::Untracked => "untracked",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub layer: Layer,
    /// Git status letter with its similarity score (`R087`), or `?` for untracked.
    pub status: String,
    /// Both names for a rename or copy; otherwise one path.
    pub paths: Vec<String>,
    /// A gitlink (submodule) on either side; its content is outside this repository.
    pub gitlink: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// merge-base..HEAD plus index, unstaged edits and non-ignored untracked files.
    Worktree,
    /// merge-base..head commits only.
    Committed,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub mode: Mode,
    pub base_ref: String,
    pub base: String,
    pub head_ref: String,
    pub head: String,
    pub merge_base: String,
    /// Commits reachable from base but not head: excluded from the diff by design.
    pub base_only: u64,
    pub head_only: u64,
    pub changes: Vec<Change>,
    /// SHA-256 over the endpoints, every raw diff record and the bytes of dirty files.
    pub fingerprint: String,
}

impl Snapshot {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// Parses `git diff --raw -z --no-abbrev` output. Unknown or truncated records fail closed.
pub fn raw_changes(raw: &[u8], layer: Layer) -> Result<Vec<Change>, String> {
    let data = std::str::from_utf8(raw)
        .map_err(|_| "non-UTF-8 Git path: inspect bytes and select full owner coverage manually")?;
    let mut fields = data.split_terminator('\0');
    let mut result = Vec::new();
    while let Some(meta) = fields.next() {
        let parts: Vec<_> = meta.strip_prefix(':').unwrap_or("").split(' ').collect();
        let [src_mode, dst_mode, _, _, status] = parts.as_slice() else {
            return Err(format!("unexpected Git raw record {meta:?}"));
        };
        let count = match status.as_bytes().first() {
            Some(b'R' | b'C') => 2,
            Some(b'A' | b'D' | b'M' | b'T' | b'U') => 1,
            _ => {
                return Err(format!(
                    "unmapped Git status {status:?}: manual owner review required"
                ));
            }
        };
        let paths: Vec<String> = fields.by_ref().take(count).map(str::to_string).collect();
        if paths.len() != count || paths.iter().any(String::is_empty) {
            return Err("incomplete Git raw record".into());
        }
        result.push(Change {
            layer,
            status: (*status).to_string(),
            paths,
            gitlink: *src_mode == "160000" || *dst_mode == "160000",
        });
    }
    Ok(result)
}

fn diff(root: &Path, endpoints: &[&str]) -> Result<Vec<u8>, String> {
    let mut args = vec![
        "diff",
        "--raw",
        "-z",
        "--no-abbrev",
        "--no-ext-diff",
        "--no-textconv",
        "--ignore-submodules=none",
        "--find-renames",
    ];
    args.extend(endpoints);
    args.push("--");
    git(root, &args)
}

fn count(root: &Path, range: &str) -> Result<u64, String> {
    text(git(root, &["rev-list", "--count", range])?)?
        .parse()
        .map_err(|e| format!("rev-list count: {e}"))
}

fn check_index_flags(root: &Path) -> Result<(), String> {
    let flags = git(root, &["ls-files", "-v", "-z"])?;
    let flagged = std::str::from_utf8(&flags)
        .map_err(|_| "non-UTF-8 Git path in the index")?
        .split_terminator('\0')
        .any(|entry| entry.starts_with('S') || entry.starts_with(|c: char| c.is_ascii_lowercase()));
    if flagged {
        return Err(
            "worktree has skip-worktree/assume-unchanged entries; inspect those inputs explicitly"
                .into(),
        );
    }
    Ok(())
}

pub fn clean_checkout_of(root: &Path, head: &str) -> Result<bool, String> {
    if commit(root, "HEAD")? != head {
        return Ok(false);
    }
    check_index_flags(root)?;
    Ok(git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=normal",
            "--ignore-submodules=none",
        ],
    )?
    .is_empty())
}

/// Reads the selected scope. Worktree mode pins HEAD; committed mode reads only commits.
pub fn snapshot(root: &Path, base_ref: &str, head_ref: Option<&str>) -> Result<Snapshot, String> {
    let mode = if head_ref.is_some() {
        Mode::Committed
    } else {
        Mode::Worktree
    };
    let head_ref = head_ref.unwrap_or("HEAD");
    let base = commit(root, base_ref)?;
    let head = commit(root, head_ref)?;
    let merge_base = text(git(root, &["merge-base", &base, &head]).map_err(|_| {
        format!("`{base_ref}` and `{head_ref}` share no merge base; pass a related --base")
    })?)?;
    let base_only = count(root, &format!("{merge_base}..{base}"))?;
    let head_only = count(root, &format!("{merge_base}..{head}"))?;
    let mut hasher = Sha256::new();
    hasher.update(format!("{mode:?}\n{base}\n{merge_base}\n{head}\n"));
    let committed = diff(root, &[&merge_base, &head])?;
    hasher.update(&committed);
    let mut changes = raw_changes(&committed, Layer::Committed)?;
    if mode == Mode::Worktree {
        check_index_flags(root)?;
        let index = diff(root, &["--cached", &head])?;
        let unstaged = diff(root, &[])?;
        let untracked = git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
        for bytes in [&index, &unstaged, &untracked] {
            hasher.update(b"\0layer\0");
            hasher.update(bytes);
        }
        changes.extend(raw_changes(&index, Layer::Index)?);
        let dirty = raw_changes(&unstaged, Layer::Worktree)?;
        let untracked: Vec<_> = std::str::from_utf8(&untracked)
            .map_err(|_| "non-UTF-8 untracked path")?
            .split_terminator('\0')
            .map(|path| Change {
                layer: Layer::Untracked,
                status: "?".into(),
                paths: vec![path.to_string()],
                gitlink: false,
            })
            .collect();
        // Raw unstaged records do not hash worktree bytes: a second edit of an already dirty
        // file would otherwise leave the fingerprint unchanged.
        for change in dirty.iter().chain(&untracked) {
            for path in &change.paths {
                hasher.update(format!("\0content\0{path}\0"));
                hasher.update(content_digest(&root.join(path))?);
            }
        }
        changes.extend(dirty);
        changes.extend(untracked);
    }
    let mut fingerprint = String::new();
    for byte in hasher.finalize() {
        write!(fingerprint, "{byte:02x}").unwrap();
    }
    Ok(Snapshot {
        mode,
        base_ref: base_ref.into(),
        base,
        head_ref: head_ref.into(),
        head,
        merge_base,
        base_only,
        head_only,
        changes,
        fingerprint,
    })
}

fn content_digest(path: &Path) -> Result<sha2::digest::Output<Sha256>, String> {
    let error = |e: std::io::Error| format!("fingerprint {}: {e}", path.display());
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(Sha256::digest(b"absent"));
        }
        Err(e) => return Err(error(e)),
    };
    if meta.is_symlink() {
        let target = std::fs::read_link(path).map_err(error)?;
        let mut hash = Sha256::new();
        hash.update(b"symlink\0");
        hash.update(target.as_os_str().as_encoded_bytes());
        return Ok(hash.finalize());
    }
    if meta.is_dir() {
        return Err(format!(
            "cannot fingerprint nested directory {}; verify the nested checkout separately",
            path.display()
        ));
    }
    if !meta.is_file() {
        return Err(format!(
            "cannot fingerprint special file {}",
            path.display()
        ));
    }
    let mut file = std::fs::File::open(path).map_err(error)?;
    let mut hash = Sha256::new();
    hash.update(b"file\0");
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer).map_err(error)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(hash.finalize())
}
