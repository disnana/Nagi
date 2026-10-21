//! Read-only, bounded observations of explicitly supplied output and Cargo roots.
//! These structural hints never authorize deletion or claim an active/last-good owner.

use std::fs::{self, Metadata};
use std::path::{Component, Path, PathBuf};

const MAX_ENTRIES: usize = 4096;
const MAX_DEPTH: usize = 64;
const MAX_ROOT_BYTES: usize = 4096;
const MAX_SAMPLES: usize = 16;
const MAX_SAMPLE_PATH_BYTES: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RootKind {
    Output,
    SharedCargo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathClass {
    PublishedGeneration,
    Staging,
    Metadata,
    SharedCargo,
    Unknown,
}

impl PathClass {
    const fn index(self) -> usize {
        match self {
            Self::PublishedGeneration => 0,
            Self::Staging => 1,
            Self::Metadata => 2,
            Self::SharedCargo => 3,
            Self::Unknown => 4,
        }
    }

    pub(crate) const fn use_hint(self) -> UseHint {
        match self {
            Self::PublishedGeneration => UseHint::GeneratedSnapshot,
            Self::Staging => UseHint::BuildInProgressOrFailed,
            Self::Metadata => UseHint::OwnerMetadataOrLock,
            Self::SharedCargo => UseHint::SharedBuildArtifact,
            Self::Unknown => UseHint::Unknown,
        }
    }

    pub(crate) const fn holding(self) -> Holding {
        match self {
            Self::PublishedGeneration | Self::Staging | Self::Metadata => {
                Holding::RequiresOwnerProtocol
            }
            Self::SharedCargo => Holding::SharedCargoNoAutomaticRemoval,
            Self::Unknown => Holding::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UseHint {
    GeneratedSnapshot,
    BuildInProgressOrFailed,
    OwnerMetadataOrLock,
    SharedBuildArtifact,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Holding {
    RequiresOwnerProtocol,
    SharedCargoNoAutomaticRemoval,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Limits {
    entries: usize,
    depth: usize,
}

impl Limits {
    pub(crate) const fn new(entries: usize, depth: usize) -> Option<Self> {
        if entries == 0 || entries > MAX_ENTRIES || depth == 0 || depth > MAX_DEPTH {
            None
        } else {
            Some(Self { entries, depth })
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ClassTotals {
    pub(crate) entries: u64,
    pub(crate) files: u64,
    pub(crate) directories: u64,
    /// Logical regular-file length. Sparse allocation and shared blocks are not measured.
    pub(crate) logical_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryKind {
    File,
    Directory,
    Hardlink,
    Symlink,
    Reparse,
    Special,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Sample {
    pub(crate) relative_path: PathBuf,
    pub(crate) class: PathClass,
    pub(crate) use_hint: UseHint,
    pub(crate) holding: Holding,
    pub(crate) kind: EntryKind,
    pub(crate) logical_bytes: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct RootReport {
    pub(crate) root: PathBuf,
    pub(crate) kind: RootKind,
    pub(crate) classes: [ClassTotals; 5],
    pub(crate) samples: Vec<Sample>,
    pub(crate) observed_entries: u64,
    pub(crate) missing_root: bool,
    pub(crate) invalid_root: bool,
    pub(crate) root_link_or_reparse: bool,
    pub(crate) root_not_directory: bool,
    pub(crate) errors: u64,
    pub(crate) entry_limit_hit: bool,
    pub(crate) depth_limit_hit: bool,
    pub(crate) overflow: bool,
    pub(crate) symlinks: u64,
    pub(crate) reparse_points: u64,
    pub(crate) hardlinks: u64,
    pub(crate) hardlink_logical_bytes: u64,
    pub(crate) hardlink_status_unknown: u64,
    pub(crate) cross_device_entries: u64,
    pub(crate) special_entries: u64,
    pub(crate) sample_omitted: u64,
}

impl RootReport {
    fn new(root: &Path, kind: RootKind) -> Self {
        Self {
            root: root.to_path_buf(),
            kind,
            classes: [ClassTotals::default(); 5],
            samples: Vec::new(),
            observed_entries: 0,
            missing_root: false,
            invalid_root: false,
            root_link_or_reparse: false,
            root_not_directory: false,
            errors: 0,
            entry_limit_hit: false,
            depth_limit_hit: false,
            overflow: false,
            symlinks: 0,
            reparse_points: 0,
            hardlinks: 0,
            hardlink_logical_bytes: 0,
            hardlink_status_unknown: 0,
            cross_device_entries: 0,
            special_entries: 0,
            sample_omitted: 0,
        }
    }

    /// Only traversal and measured logical-file coverage; never ownership or physical bytes.
    pub(crate) fn enumeration_complete(&self) -> bool {
        !self.missing_root
            && !self.invalid_root
            && !self.root_link_or_reparse
            && !self.root_not_directory
            && self.errors == 0
            && !self.entry_limit_hit
            && !self.depth_limit_hit
            && !self.overflow
            && self.cross_device_entries == 0
    }

    pub(crate) fn logical_bytes_complete(&self) -> bool {
        self.enumeration_complete()
            && self.symlinks == 0
            && self.reparse_points == 0
            && self.special_entries == 0
    }

    fn issue(&mut self) {
        if !checked_add(&mut self.errors, 1) {
            self.overflow = true;
        }
    }

    fn observe(&mut self, relative: &Path, class: PathClass, kind: EntryKind, bytes: Option<u64>) {
        let totals = &mut self.classes[class.index()];
        let mut valid = checked_add(&mut totals.entries, 1);
        match kind {
            EntryKind::File | EntryKind::Hardlink => {
                valid &= checked_add(&mut totals.files, 1);
            }
            EntryKind::Directory => valid &= checked_add(&mut totals.directories, 1),
            _ => {}
        }
        if let Some(bytes) = bytes {
            valid &= checked_add(&mut totals.logical_bytes, bytes);
        }
        if !valid {
            self.overflow = true;
        }
        if self.samples.len() < MAX_SAMPLES
            && relative.as_os_str().as_encoded_bytes().len() <= MAX_SAMPLE_PATH_BYTES
        {
            self.samples.push(Sample {
                relative_path: relative.to_path_buf(),
                class,
                use_hint: class.use_hint(),
                holding: class.holding(),
                kind,
                logical_bytes: bytes,
            });
        } else if !checked_add(&mut self.sample_omitted, 1) {
            self.overflow = true;
        }
    }
}

#[derive(Debug)]
pub(crate) struct Inventory {
    pub(crate) output: RootReport,
    pub(crate) cargo: RootReport,
    /// Lexical overlap is a warning; different paths may still alias on disk.
    pub(crate) roots_overlap_lexically: bool,
}

/// Caller-supplied roots only. This does not discover project/env defaults or combine volumes.
pub(crate) fn inspect(output: &Path, cargo: &Path, limits: Limits) -> Inventory {
    Inventory {
        output: scan_root(output, RootKind::Output, limits),
        cargo: scan_root(cargo, RootKind::SharedCargo, limits),
        roots_overlap_lexically: output.starts_with(cargo) || cargo.starts_with(output),
    }
}

pub(crate) fn scan_root(root: &Path, kind: RootKind, limits: Limits) -> RootReport {
    let mut report = RootReport::new(root, kind);
    let Some(root_metadata) = validate_root(root, &mut report) else {
        return report;
    };
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    'walk: while let Some((directory, depth)) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => {
                report.issue();
                continue;
            }
        };
        for entry in entries {
            if report.observed_entries >= limits.entries as u64 {
                report.entry_limit_hit = true;
                break 'walk;
            }
            if !checked_add(&mut report.observed_entries, 1) {
                report.overflow = true;
                break 'walk;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    report.issue();
                    continue;
                }
            };
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(root) else {
                report.issue();
                continue;
            };
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => {
                    report.issue();
                    continue;
                }
            };
            if metadata.file_type().is_symlink() {
                if !checked_add(&mut report.symlinks, 1) {
                    report.overflow = true;
                }
                report.observe(relative, PathClass::Unknown, EntryKind::Symlink, None);
                continue;
            }
            if is_reparse(&metadata) {
                if !checked_add(&mut report.reparse_points, 1) {
                    report.overflow = true;
                }
                report.observe(relative, PathClass::Unknown, EntryKind::Reparse, None);
                continue;
            }
            if different_device(&root_metadata, &metadata) {
                if !checked_add(&mut report.cross_device_entries, 1) {
                    report.overflow = true;
                }
                report.observe(relative, PathClass::Unknown, EntryKind::Special, None);
                continue;
            }
            if metadata.is_dir() {
                report.observe(
                    relative,
                    classify(kind, relative),
                    EntryKind::Directory,
                    None,
                );
                if depth + 1 >= limits.depth {
                    report.depth_limit_hit = true;
                } else {
                    pending.push((path, depth + 1));
                }
            } else if metadata.is_file() {
                let (class, entry_kind) = if known_hardlink(&metadata) {
                    if !checked_add(&mut report.hardlinks, 1)
                        || !checked_add(&mut report.hardlink_logical_bytes, metadata.len())
                    {
                        report.overflow = true;
                    }
                    (PathClass::Unknown, EntryKind::Hardlink)
                } else {
                    #[cfg(not(unix))]
                    if !checked_add(&mut report.hardlink_status_unknown, 1) {
                        report.overflow = true;
                    }
                    (classify(kind, relative), EntryKind::File)
                };
                report.observe(relative, class, entry_kind, Some(metadata.len()));
            } else {
                if !checked_add(&mut report.special_entries, 1) {
                    report.overflow = true;
                }
                report.observe(relative, PathClass::Unknown, EntryKind::Special, None);
            }
        }
    }
    report
}

fn validate_root(root: &Path, report: &mut RootReport) -> Option<Metadata> {
    if !root.is_absolute() || root.as_os_str().as_encoded_bytes().len() > MAX_ROOT_BYTES {
        report.invalid_root = true;
        return None;
    }
    let mut path = PathBuf::new();
    let mut components = 0;
    for component in root.components() {
        components += 1;
        if components > MAX_DEPTH {
            report.invalid_root = true;
            return None;
        }
        match component {
            Component::Prefix(_) | Component::RootDir => path.push(component.as_os_str()),
            Component::Normal(_) => {
                path.push(component.as_os_str());
                let metadata = match fs::symlink_metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        if error.kind() == std::io::ErrorKind::NotFound {
                            report.missing_root = true;
                        } else {
                            report.issue();
                        }
                        return None;
                    }
                };
                if metadata.file_type().is_symlink() || is_reparse(&metadata) {
                    report.root_link_or_reparse = true;
                    return None;
                }
            }
            Component::CurDir | Component::ParentDir => {
                report.invalid_root = true;
                return None;
            }
        }
    }
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(_) => {
            report.issue();
            return None;
        }
    };
    if !metadata.is_dir() {
        report.root_not_directory = true;
        return None;
    }
    Some(metadata)
}

fn classify(kind: RootKind, relative: &Path) -> PathClass {
    if kind == RootKind::SharedCargo {
        return PathClass::SharedCargo;
    }
    if relative == Path::new(".nagi-write.lock") {
        return PathClass::Metadata;
    }
    let mut parts = relative.components();
    if parts.next().and_then(as_text) != Some(".nagi")
        || parts.next().and_then(as_text) != Some("apps")
    {
        return PathClass::Unknown;
    }
    let Some(app) = parts.next().and_then(as_text) else {
        return PathClass::Unknown;
    };
    if !valid_app(app) {
        return PathClass::Unknown;
    }
    match parts.next().and_then(as_text) {
        Some("generations") => match parts.next().and_then(as_text) {
            Some(name) if name.strip_prefix(".staging-").is_some_and(valid_generation) => {
                PathClass::Staging
            }
            Some(name) if valid_generation(name) => PathClass::PublishedGeneration,
            _ => PathClass::Unknown,
        },
        Some("latest.json") if parts.next().is_none() => PathClass::Metadata,
        Some(name)
            if name
                .strip_prefix(".latest-")
                .and_then(|value| value.strip_suffix(".tmp"))
                .is_some_and(valid_generation)
                && parts.next().is_none() =>
        {
            PathClass::Metadata
        }
        Some("run-retention") => match parts.next().and_then(as_text) {
            Some(name)
                if name
                    .strip_suffix(".lease")
                    .or_else(|| name.strip_suffix(".exported"))
                    .is_some_and(valid_generation)
                    && parts.next().is_none() =>
            {
                PathClass::Metadata
            }
            _ => PathClass::Unknown,
        },
        _ => PathClass::Unknown,
    }
}

fn as_text(component: Component<'_>) -> Option<&str> {
    component.as_os_str().to_str()
}

fn valid_app(name: &str) -> bool {
    let Some((stem, hash)) = name.rsplit_once('-') else {
        return false;
    };
    stem.starts_with("nagi-")
        && stem.len() > "nagi-".len()
        && hash.len() == 16
        && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_generation(name: &str) -> bool {
    let mut parts = name.split('-');
    parts.next() == Some("g")
        && (0..3).all(|_| {
            parts.next().is_some_and(|part| {
                !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        })
        && parts.next().is_none()
}

fn checked_add(value: &mut u64, increment: u64) -> bool {
    let Some(next) = value.checked_add(increment) else {
        return false;
    };
    *value = next;
    true
}

#[cfg(windows)]
fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_: &Metadata) -> bool {
    false
}

#[cfg(unix)]
fn known_hardlink(metadata: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink() > 1
}

#[cfg(not(unix))]
fn known_hardlink(_: &Metadata) -> bool {
    false
}

#[cfg(unix)]
fn different_device(root: &Metadata, entry: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    root.dev() != entry.dev()
}

#[cfg(not(unix))]
fn different_device(_: &Metadata, _: &Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    const APP: &str = "nagi-main-0123456789abcdef";
    const GENERATION: &str = "g-a-b-c";

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            for _ in 0..100 {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let name = format!(
                    "nagi-cache-inventory-{}-{now}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                );
                let path = std::env::temp_dir().join(name);
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("cannot create test directory: {error}"),
                }
            }
            panic!("cannot reserve an exclusive test directory");
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn file(&self, relative: &str, bytes: u64) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            File::create(&path).unwrap().set_len(bytes).unwrap();
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn limits(entries: usize, depth: usize) -> Limits {
        Limits::new(entries, depth).unwrap()
    }

    #[test]
    fn limits_reject_unbounded_or_empty_requests() {
        assert!(Limits::new(0, 1).is_none());
        assert!(Limits::new(MAX_ENTRIES + 1, 1).is_none());
        assert!(Limits::new(1, 0).is_none());
        assert!(Limits::new(1, MAX_DEPTH + 1).is_none());
        assert_eq!(limits(1, 1).entries, 1);
    }

    #[test]
    fn empty_root_is_complete_without_claiming_owner_or_physical_bytes() {
        let dir = TestDir::new();
        let report = scan_root(dir.path(), RootKind::Output, limits(10, 3));
        assert_eq!(report.root, dir.path());
        assert!(report.enumeration_complete());
        assert!(report.logical_bytes_complete());
        assert_eq!(report.observed_entries, 0);
        assert_eq!(report.hardlink_status_unknown, 0);
        assert!(report
            .classes
            .iter()
            .all(|totals| totals.logical_bytes == 0));
    }

    #[test]
    fn structural_generation_staging_and_metadata_are_distinct() {
        let dir = TestDir::new();
        dir.file(
            &format!(".nagi/apps/{APP}/generations/{GENERATION}/run"),
            17,
        );
        dir.file(
            &format!(".nagi/apps/{APP}/generations/.staging-g-d-e-f/src/main.rs"),
            9,
        );
        dir.file(&format!(".nagi/apps/{APP}/latest.json"), 3);
        dir.file(&format!(".nagi/apps/{APP}/.latest-g-a-b-c.tmp"), 5);
        dir.file(
            &format!(".nagi/apps/{APP}/run-retention/{GENERATION}.lease"),
            7,
        );
        dir.file(".nagi-write.lock", 0);
        let report = scan_root(dir.path(), RootKind::Output, limits(100, 12));
        assert!(report.enumeration_complete());
        assert_eq!(
            report.classes[PathClass::PublishedGeneration.index()].logical_bytes,
            17
        );
        assert_eq!(report.classes[PathClass::Staging.index()].logical_bytes, 9);
        assert_eq!(
            report.classes[PathClass::Metadata.index()].logical_bytes,
            15
        );
        assert!(report.samples.iter().any(|sample| {
            sample.class == PathClass::PublishedGeneration
                && sample.kind == EntryKind::File
                && sample.logical_bytes == Some(17)
                && sample.holding == Holding::RequiresOwnerProtocol
        }));
        assert_eq!(
            PathClass::PublishedGeneration.holding(),
            Holding::RequiresOwnerProtocol
        );
        assert_eq!(
            PathClass::Staging.use_hint(),
            UseHint::BuildInProgressOrFailed
        );
    }

    #[test]
    fn shared_cargo_is_reported_separately_and_never_auto_removable() {
        let dir = TestDir::new();
        let file = dir.file("release/deps/library.rlib", 4096);
        let before = fs::metadata(&file).unwrap().len();
        let report = scan_root(dir.path(), RootKind::SharedCargo, limits(20, 5));
        assert!(report.enumeration_complete());
        assert_eq!(
            report.classes[PathClass::SharedCargo.index()].logical_bytes,
            before
        );
        assert_eq!(
            PathClass::SharedCargo.holding(),
            Holding::SharedCargoNoAutomaticRemoval
        );
        assert_eq!(fs::metadata(&file).unwrap().len(), before);
    }

    #[test]
    fn unknown_names_do_not_inherit_generation_ownership() {
        let dir = TestDir::new();
        dir.file(".nagi/apps/foreign/generations/g-a-b-c/file", 11);
        dir.file("Cargo.toml", 3);
        let report = scan_root(dir.path(), RootKind::Output, limits(20, 8));
        assert_eq!(report.classes[PathClass::Unknown.index()].logical_bytes, 14);
        assert_eq!(PathClass::Unknown.holding(), Holding::Unknown);
        assert!(report.logical_bytes_complete());
    }

    #[test]
    fn missing_root_is_not_zero_byte_success() {
        let dir = TestDir::new();
        let report = scan_root(&dir.path().join("missing"), RootKind::Output, limits(10, 2));
        assert!(report.missing_root);
        assert!(!report.enumeration_complete());
        assert!(!report.logical_bytes_complete());
    }

    #[test]
    fn recorded_io_error_is_never_complete_zero_byte_success() {
        let mut report = RootReport::new(Path::new("/unreadable"), RootKind::Output);
        report.issue();
        assert_eq!(report.errors, 1);
        assert!(!report.enumeration_complete());
        assert!(!report.logical_bytes_complete());
    }

    #[test]
    fn plain_file_root_is_not_a_directory_inventory() {
        let dir = TestDir::new();
        let path = dir.file("root-file", 10);
        let report = scan_root(&path, RootKind::Output, limits(10, 2));
        assert!(report.root_not_directory);
        assert!(!report.enumeration_complete());
    }

    #[test]
    fn relative_root_is_rejected_before_any_traversal() {
        let report = scan_root(
            Path::new("relative/output"),
            RootKind::Output,
            limits(10, 2),
        );
        assert!(report.invalid_root);
        assert_eq!(report.observed_entries, 0);
        assert!(!report.enumeration_complete());
    }

    #[test]
    fn entry_budget_stops_without_complete_zero_byte_report() {
        let dir = TestDir::new();
        for index in 0..4 {
            dir.file(&format!("file-{index}"), 10);
        }
        let report = scan_root(dir.path(), RootKind::Output, limits(2, 2));
        assert!(report.entry_limit_hit);
        assert_eq!(report.observed_entries, 2);
        assert!(!report.enumeration_complete());
    }

    #[test]
    fn depth_budget_marks_unvisited_descendants() {
        let dir = TestDir::new();
        dir.file("nested/value", 8);
        let report = scan_root(dir.path(), RootKind::Output, limits(10, 1));
        assert!(report.depth_limit_hit);
        assert!(!report.enumeration_complete());
        assert_eq!(report.classes[PathClass::Unknown.index()].logical_bytes, 0);
    }

    #[test]
    fn samples_are_bounded_and_relative() {
        let dir = TestDir::new();
        for index in 0..20 {
            dir.file(&format!("file-{index}"), 1);
        }
        let report = scan_root(dir.path(), RootKind::Output, limits(30, 2));
        assert_eq!(report.samples.len(), MAX_SAMPLES);
        assert_eq!(report.sample_omitted, 4);
        assert!(report
            .samples
            .iter()
            .all(|sample| !sample.relative_path.is_absolute()));
    }

    #[test]
    fn overlapping_explicit_roots_have_no_combined_total() {
        let dir = TestDir::new();
        fs::create_dir(dir.path().join("cargo")).unwrap();
        let report = inspect(dir.path(), &dir.path().join("cargo"), limits(10, 2));
        assert!(report.roots_overlap_lexically);
        assert_eq!(report.output.kind, RootKind::Output);
        assert_eq!(report.cargo.kind, RootKind::SharedCargo);
    }

    #[test]
    fn arithmetic_overflow_is_detected_without_wrapping() {
        let mut value = u64::MAX;
        assert!(!checked_add(&mut value, 1));
        assert_eq!(value, u64::MAX);
    }

    #[cfg(unix)]
    #[test]
    fn discovered_symlink_is_not_followed_or_counted_as_file_bytes() {
        use std::os::unix::fs::symlink;
        let dir = TestDir::new();
        let outside = TestDir::new();
        outside.file("private", 8192);
        symlink(outside.path().join("private"), dir.path().join("link")).unwrap();
        let report = scan_root(dir.path(), RootKind::Output, limits(10, 2));
        assert_eq!(report.symlinks, 1);
        assert!(!report.logical_bytes_complete());
        assert_eq!(report.classes[PathClass::Unknown.index()].logical_bytes, 0);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_ancestor_of_explicit_root_is_refused() {
        use std::os::unix::fs::symlink;
        let dir = TestDir::new();
        fs::create_dir(dir.path().join("actual")).unwrap();
        symlink(dir.path().join("actual"), dir.path().join("alias")).unwrap();
        let report = scan_root(
            &dir.path().join("alias/child"),
            RootKind::Output,
            limits(10, 2),
        );
        assert!(report.root_link_or_reparse);
        assert!(!report.enumeration_complete());
    }

    #[cfg(unix)]
    #[test]
    fn hardlinks_are_separate_logical_bytes_not_physical_savings() {
        let dir = TestDir::new();
        let first = dir.file("one", 10);
        fs::hard_link(&first, dir.path().join("two")).unwrap();
        let report = scan_root(dir.path(), RootKind::Output, limits(10, 2));
        assert_eq!(report.hardlinks, 2);
        assert_eq!(report.hardlink_logical_bytes, 20);
        assert_eq!(report.classes[PathClass::Unknown.index()].logical_bytes, 20);
        assert!(report
            .samples
            .iter()
            .all(|sample| sample.kind == EntryKind::Hardlink));
    }
}
