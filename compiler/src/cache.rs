//! Read-only command-line adapter for bounded cache observations.

use crate::cache_inventory::{self, Limits, RootKind, RootReport};
use serde_json::{json, Value};
use std::path::PathBuf;

const MAX_ENTRIES: usize = 4096;
const MAX_DEPTH: usize = 64;
// Keep CLI root inputs aligned with the inventory foundation's MAX_ROOT_BYTES.
const MAX_ROOT_BYTES: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
struct InspectOptions {
    output_root: PathBuf,
    cargo_root: PathBuf,
    max_entries: usize,
    max_depth: usize,
}

fn parse_inspect_args(args: &[String]) -> Result<InspectOptions, String> {
    if args.first().map(String::as_str) != Some("cache")
        || args.get(1).map(String::as_str) != Some("inspect")
    {
        return Err("expected `cache inspect`".into());
    }

    let mut output_root = None;
    let mut cargo_root = None;
    let mut max_entries = None;
    let mut max_depth = None;
    let mut index = 2;
    while index < args.len() {
        let option = args[index].as_str();
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| format!("{option} requires a value"))?;
        index += 1;
        match option {
            "--out-root" => set_root(&mut output_root, value, option)?,
            "--cargo-root" => set_root(&mut cargo_root, value, option)?,
            "--max-entries" => {
                set_limit(&mut max_entries, value, option, 1, MAX_ENTRIES)?;
            }
            "--max-depth" => set_limit(&mut max_depth, value, option, 1, MAX_DEPTH)?,
            _ => return Err(format!("unknown cache inspect option: {option}")),
        }
    }

    Ok(InspectOptions {
        output_root: output_root.ok_or("cache inspect requires --out-root")?,
        cargo_root: cargo_root.ok_or("cache inspect requires --cargo-root")?,
        max_entries: max_entries.ok_or("cache inspect requires --max-entries")?,
        max_depth: max_depth.ok_or("cache inspect requires --max-depth")?,
    })
}

fn set_root(slot: &mut Option<PathBuf>, value: &str, option: &str) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("{option} may be specified only once"));
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{option} must be an absolute path"));
    }
    if path.as_os_str().as_encoded_bytes().len() > MAX_ROOT_BYTES {
        return Err(format!(
            "{option} must be at most {MAX_ROOT_BYTES} encoded path bytes"
        ));
    }
    *slot = Some(path);
    Ok(())
}

fn set_limit(
    slot: &mut Option<usize>,
    value: &str,
    option: &str,
    minimum: usize,
    maximum: usize,
) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("{option} may be specified only once"));
    }
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{option} must be an integer from {minimum} to {maximum}"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!(
            "{option} must be an integer from {minimum} to {maximum}"
        ));
    }
    *slot = Some(parsed);
    Ok(())
}

/// Parse first, then inspect only the two explicit roots, and print one JSON record.
pub(crate) fn cli(args: &[String]) -> Result<(), String> {
    let options = parse_inspect_args(args)?;
    let limits = Limits::new(options.max_entries, options.max_depth)
        .expect("CLI limits were checked before inspection");
    let inventory = cache_inventory::inspect(&options.output_root, &options.cargo_root, limits);
    let incomplete = [&inventory.output, &inventory.cargo]
        .into_iter()
        .any(|root| !root.enumeration_complete() || !root.logical_bytes_complete());
    let report = inventory_json(
        &inventory.output,
        &inventory.cargo,
        inventory.roots_overlap_lexically,
    );
    let encoded = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("cannot encode cache inspection: {error}"))?;
    println!("{encoded}");
    if incomplete {
        return Err("cache inspect incomplete; see the JSON reason counters".into());
    }
    Ok(())
}

fn inventory_json(output: &RootReport, cargo: &RootReport, roots_overlap_lexically: bool) -> Value {
    json!({
        "format_version": 1,
        "roots": {
            "output": root_json(output),
            "shared_cargo": root_json(cargo),
        },
        "roots_overlap_lexically": roots_overlap_lexically,
        "limitations": [
            "Classification is a structural hint, not proof of ownership or use.",
            "Hardlinks are counted as paths; logical bytes may count shared file contents more than once.",
            "Only logical regular-file lengths are measured; physical allocation, free space, and reclaimable bytes are not measured.",
            "Roots can alias on disk even when lexical overlap is false; results must not be summed as unique disk consumption.",
            "Observed symlinks and reparse points are skipped or rejected, but path check/use races remain.",
            "Portable filesystem traversal is not an atomic snapshot or a race-safe no-follow boundary."
        ]
    })
}

fn root_json(report: &RootReport) -> Value {
    let classes = [
        "published_generation",
        "staging",
        "metadata",
        "shared_cargo",
        "unknown",
    ];
    let classes: serde_json::Map<String, Value> = classes
        .into_iter()
        .zip(report.classes.iter())
        .map(|(name, totals)| {
            (
                name.into(),
                json!({
                    "entries": totals.entries,
                    "files": totals.files,
                    "directories": totals.directories,
                    "logical_regular_file_bytes": totals.logical_bytes,
                }),
            )
        })
        .collect();
    let samples: Vec<_> = report
        .samples
        .iter()
        .map(|sample| {
            json!({
                "relative_path": sample.relative_path.to_string_lossy(),
                "relative_path_os": crate::generation::os_path(&sample.relative_path),
                "class": path_class_name(sample.class),
                "use_hint": use_hint_name(sample.use_hint),
                "holding": holding_name(sample.holding),
                "entry_kind": entry_kind_name(sample.kind),
                "logical_regular_file_bytes": sample.logical_bytes,
            })
        })
        .collect();

    json!({
        "root": report.root.to_string_lossy(),
        "root_os": crate::generation::os_path(&report.root),
        "kind": match report.kind {
            RootKind::Output => "output",
            RootKind::SharedCargo => "shared_cargo",
        },
        "observed_entries": report.observed_entries,
        "classes": classes,
        "samples": samples,
        "reasons": {
            "missing_root": report.missing_root,
            "invalid_root": report.invalid_root,
            "root_link_or_reparse": report.root_link_or_reparse,
            "root_not_directory": report.root_not_directory,
            "errors": report.errors,
            "entry_limit_hit": report.entry_limit_hit,
            "depth_limit_hit": report.depth_limit_hit,
            "overflow": report.overflow,
            "symlinks": report.symlinks,
            "reparse_points": report.reparse_points,
            "hardlinks": report.hardlinks,
            "hardlink_logical_bytes": report.hardlink_logical_bytes,
            "hardlink_status_unknown": report.hardlink_status_unknown,
            "cross_device_entries": report.cross_device_entries,
            "special_entries": report.special_entries,
            "sample_omitted": report.sample_omitted,
        },
        "enumeration_complete": report.enumeration_complete(),
        "logical_bytes_complete": report.logical_bytes_complete(),
    })
}

fn path_class_name(class: cache_inventory::PathClass) -> &'static str {
    match class {
        cache_inventory::PathClass::PublishedGeneration => "published_generation",
        cache_inventory::PathClass::Staging => "staging",
        cache_inventory::PathClass::Metadata => "metadata",
        cache_inventory::PathClass::SharedCargo => "shared_cargo",
        cache_inventory::PathClass::Unknown => "unknown",
    }
}

fn use_hint_name(hint: cache_inventory::UseHint) -> &'static str {
    match hint {
        cache_inventory::UseHint::GeneratedSnapshot => "generated_snapshot",
        cache_inventory::UseHint::BuildInProgressOrFailed => "build_in_progress_or_failed",
        cache_inventory::UseHint::OwnerMetadataOrLock => "owner_metadata_or_lock",
        cache_inventory::UseHint::SharedBuildArtifact => "shared_build_artifact",
        cache_inventory::UseHint::Unknown => "unknown",
    }
}

fn holding_name(holding: cache_inventory::Holding) -> &'static str {
    match holding {
        cache_inventory::Holding::RequiresOwnerProtocol => "requires_owner_protocol",
        cache_inventory::Holding::SharedCargoNoAutomaticRemoval => {
            "shared_cargo_no_automatic_removal"
        }
        cache_inventory::Holding::Unknown => "unknown",
    }
}

fn entry_kind_name(kind: cache_inventory::EntryKind) -> &'static str {
    match kind {
        cache_inventory::EntryKind::File => "file",
        cache_inventory::EntryKind::Directory => "directory",
        cache_inventory::EntryKind::Hardlink => "hardlink",
        cache_inventory::EntryKind::Symlink => "symlink",
        cache_inventory::EntryKind::Reparse => "reparse",
        cache_inventory::EntryKind::Special => "special",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(options: Vec<String>) -> Vec<String> {
        let mut args = vec!["cache".into(), "inspect".into()];
        args.extend(options);
        args
    }

    fn valid_options() -> Vec<String> {
        let root = std::env::temp_dir();
        vec![
            "--out-root".into(),
            root.join("output").to_string_lossy().into_owned(),
            "--cargo-root".into(),
            root.join("cargo").to_string_lossy().into_owned(),
            "--max-entries".into(),
            "1".into(),
            "--max-depth".into(),
            "1".into(),
        ]
    }

    #[test]
    fn parser_requires_all_four_options_and_accepts_bounded_values() {
        let root = std::env::temp_dir();
        let parsed = parse_inspect_args(&args(valid_options())).unwrap();
        assert_eq!(parsed.output_root, root.join("output"));
        assert_eq!(parsed.cargo_root, root.join("cargo"));
        assert_eq!(parsed.max_entries, 1);
        assert_eq!(parsed.max_depth, 1);
        for omitted in [0, 2, 4, 6] {
            let mut options = valid_options();
            options.drain(omitted..omitted + 2);
            assert!(parse_inspect_args(&args(options)).is_err());
        }
    }

    #[test]
    fn parser_rejects_relative_roots_duplicates_and_invalid_limits() {
        let mut options = valid_options();
        options[1] = "relative".into();
        assert!(parse_inspect_args(&args(options)).is_err());
        let mut options = valid_options();
        options.extend(["--max-depth".into(), "1".into()]);
        assert!(parse_inspect_args(&args(options)).is_err());
        for (flag, value) in [
            ("--max-entries", "0"),
            ("--max-entries", "4097"),
            ("--max-depth", "65"),
            ("--max-depth", "x"),
        ] {
            let mut options = valid_options();
            let position = options.iter().position(|item| *item == flag).unwrap();
            options[position + 1] = value.into();
            assert!(
                parse_inspect_args(&args(options)).is_err(),
                "{flag} {value}"
            );
        }
    }

    #[test]
    fn parser_accepts_exact_root_byte_limit_and_rejects_larger_encoded_paths() {
        fn absolute_path_with_encoded_len(target: usize, suffix: &str) -> PathBuf {
            let base = std::env::temp_dir().join("cache-inspect-root");
            let mut value = base.to_string_lossy().into_owned();
            value.push_str(suffix);
            let current = PathBuf::from(&value).as_os_str().as_encoded_bytes().len();
            assert!(current < target);
            value.push_str(&"x".repeat(target - current));
            let path = PathBuf::from(value);
            assert_eq!(path.as_os_str().as_encoded_bytes().len(), target);
            assert!(path.is_absolute());
            path
        }

        let exact = absolute_path_with_encoded_len(4096, "é");
        let oversized_ascii = absolute_path_with_encoded_len(4097, "");
        let oversized_non_ascii = absolute_path_with_encoded_len(4097, "é");

        let options_for = |output: &std::path::Path| {
            let mut options = valid_options();
            options[1] = output.to_string_lossy().into_owned();
            options
        };
        assert!(parse_inspect_args(&args(options_for(&exact))).is_ok());

        for path in [&oversized_ascii, &oversized_non_ascii] {
            let result = parse_inspect_args(&args(options_for(path)));
            assert!(result.unwrap_err().contains("4096 encoded path bytes"));
        }
        assert_eq!(
            oversized_non_ascii.as_os_str().as_encoded_bytes().len(),
            4097
        );
    }

    #[test]
    fn parser_rejects_unknown_options_and_subcommands() {
        let mut options = valid_options();
        options.extend(["--project".into(), "manifest".into()]);
        assert!(parse_inspect_args(&args(options)).is_err());
        let mut extra_subcommand = args(valid_options());
        extra_subcommand.push("prune".into());
        assert!(parse_inspect_args(&extra_subcommand).is_err());
        assert!(parse_inspect_args(&["cache".into(), "prune".into()]).is_err());
    }

    #[test]
    fn root_serialization_preserves_the_path_os_units() {
        let path = std::env::temp_dir().join("cache-root");
        let report =
            cache_inventory::scan_root(&path, RootKind::Output, Limits::new(1, 1).unwrap());
        let serialized = root_json(&report);
        assert_eq!(serialized["root_os"], crate::generation::os_path(&path));
        let encoded = serde_json::to_string(&inventory_json(&report, &report, true)).unwrap();
        assert!(!encoded.contains("safe_to_delete"));
        assert!(!encoded.contains("eligible"));
    }
}
