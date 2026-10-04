#!/usr/bin/env bash
# Remove only installer-managed commands and unchanged verified distributions.
set -euo pipefail
prefix="$HOME/.local/share/nagi"
bin_dir="$HOME/.local/bin"
profile=''
no_path=0
dry_run=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --prefix) prefix=${2:?Missing install directory}; shift 2 ;;
        --bin-dir) bin_dir=${2:?Missing bin directory}; shift 2 ;;
        --profile) profile=${2:?Missing shell profile}; shift 2 ;;
        --no-path) no_path=1; shift ;;
        --dry-run) dry_run=1; shift ;;
        --help) printf '%s\n' 'Usage: uninstall.sh [--prefix DIR] [--bin-dir DIR] [--profile FILE] [--no-path] [--dry-run]'; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done
case "$(uname -s)/$(uname -m)" in
    Linux/x86_64) platform=linux-x86_64 ;;
    Darwin/arm64) platform=macos-arm64 ;;
    Darwin/x86_64) platform=macos-x86_64 ;;
    *) echo 'Supported: Linux x86_64, macOS arm64/x86_64. Use uninstall.ps1 on Windows.' >&2; exit 1 ;;
esac
for tool in curl tar cmp find sort tr; do command -v "$tool" >/dev/null || { echo "Required: $tool" >&2; exit 1; }; done
if command -v sha256sum >/dev/null; then
    hash() { sha256sum "$1" | cut -d ' ' -f 1; }
elif command -v shasum >/dev/null; then
    hash() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
else echo 'Required: sha256sum or shasum' >&2; exit 1
fi

# Reject linked ancestors as well as linked roots before reading or deleting.
plain_directory() (
    local requested=$1 current=/ component
    [[ "$requested" == /* ]] || requested="$PWD/$requested"
    local IFS=/
    read -r -a components <<< "$requested"
    for component in "${components[@]}"; do
        case "$component" in ''|.) continue ;; ..) current=${current%/*}; [ -n "$current" ] || current=/; continue ;; esac
        current="${current%/}/$component"
        [ ! -L "$current" ] && [ -d "$current" ] || return 1
    done
    cd "$current" && pwd -P
)
if [ ! -e "$prefix" ] && [ ! -L "$prefix" ]; then
    printf '[OK] No installation directory: %s\n' "$prefix"
    exit 0
fi
prefix=$(plain_directory "$prefix") || { echo 'Install directory contains a link or is not a directory; kept unchanged.' >&2; exit 1; }
lock="$prefix/.install-lock"
mkdir "$lock" 2>/dev/null || { echo "Another installer/uninstaller is running, or a previous run was interrupted: $lock" >&2; exit 1; }
work=''
cleanup() {
    local status=$?
    trap - EXIT
    [ -z "$work" ] || rm -rf "$work"
    rmdir "$lock"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
work=$(mktemp -d "$prefix/.uninstall.XXXXXX")
download() {
    curl --fail --location --proto '=https' --proto-redir '=https' --tlsv1.2 --retry 3 --connect-timeout 15 --max-time 300 "$@"
}
fetch_release() {
    local release=$1 folder=$2 name="nagi-$1-$platform" file entry checksum expected filename extra
    local asset="$name.tar.gz" base="https://github.com/disnana/Nagi/releases/download/nagi-v$release"
    mkdir "$folder" || return 1
    for file in "$asset" "$asset.sha256"; do
        download "$base/$file" -o "$folder/$file" || return 1
    done
    checksum=$(cat "$folder/$asset.sha256") || return 1
    read -r expected filename extra <<< "$checksum"
    [[ "$expected" =~ ^[0-9a-f]{64}$ && "$filename" == "$asset" && -z "$extra" && "$checksum" != *$'\n'* ]] || return 1
    [ "$(hash "$folder/$asset")" = "$expected" ] || return 1
    tar -tzf "$folder/$asset" > "$folder/entries" || return 1
    while IFS= read -r entry; do
        [[ "$entry" == "$name/"* && "$entry" != *[^a-zA-Z0-9._/-]* ]] || return 1
        case "/$entry/" in *'/../'*|*'/./'*) return 1 ;; esac
    done < "$folder/entries"
    tar -tvzf "$folder/$asset" > "$folder/modes" || return 1
    while IFS= read -r entry; do
        case "$entry" in [-d]*) ;; *) return 1 ;; esac
    done < "$folder/modes"
    tar -xzf "$folder/$asset" --no-same-owner --no-same-permissions -C "$folder" || return 1
    for file in nagic runtime/Cargo.toml runtime/src/lib.rs release.json LICENSE README.txt; do
        [ -f "$folder/$name/$file" ] || return 1
    done
    grep -Fqx "  \"version\": \"$release\"," "$folder/$name/release.json" &&
        grep -Fqx "  \"platform\": \"$platform\"," "$folder/$name/release.json"
}
snapshot() (
    [ -d "$1" ] && [ ! -L "$1" ] || exit 1
    cd "$1" || exit 1
    find . -print | LC_ALL=C sort | while IFS= read -r file; do
        [[ "$file" != *[^a-zA-Z0-9._/-]* ]] && [ ! -L "$file" ] || exit 1
        if [ -d "$file" ]; then printf 'D %s\n' "$file"
        elif [ -f "$file" ]; then
            digest=$(hash "$file") || exit 1
            printf 'F %s %s\n' "$digest" "$file"
        else exit 1
        fi
    done
)
report_remove() {
    if [ "$dry_run" -eq 1 ]; then printf 'Would remove %s: %s\n' "$1" "$2"
    else printf 'Removed %s: %s\n' "$1" "$2"
    fi
}
if [ -e "$bin_dir" ] || [ -L "$bin_dir" ]; then
    bin_dir=$(plain_directory "$bin_dir") || { echo 'Command directory contains a link or is not a directory; kept unchanged.' >&2; exit 1; }
    link="$bin_dir/nagic"
    if [ -L "$link" ]; then
        target=$(readlink "$link")
        stem=${target#"$prefix/"}
        if [[ "$target" == "$prefix/"* && "$stem" =~ ^nagi-[0-9]+\.[0-9]+\.[0-9]+-$platform/nagic$ ]]; then
            [ "$dry_run" -eq 1 ] || rm "$link"
            report_remove command "$link"
        else printf 'Kept foreign command link: %s\n' "$link" >&2
        fi
    elif [ -e "$link" ]; then printf 'Kept existing command: %s\n' "$link" >&2
    fi
else
    # There is no command to detach. Canonicalize existing parents for the
    # exact profile stanza; do not create the requested command directory.
    parent=$(plain_directory "$(dirname "$bin_dir")") || { echo 'Command directory parent is missing or linked; kept profiles unchanged.' >&2; exit 1; }
    bin_dir="${parent%/}/$(basename "$bin_dir")"
fi
if [ "$no_path" -eq 0 ]; then
    quoted=${bin_dir//\'/\'\\\'\'}
    quoted="'$quoted'"
    line="case \"\$PATH\" in $quoted|$quoted:*) ;; *) export PATH=$quoted:\"\$PATH\" ;; esac # Nagi installer"
    if [ -n "$profile" ]; then profiles=("$profile")
    else profiles=("$HOME/.profile" "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.bashrc" "$HOME/.zprofile" "$HOME/.zshrc")
    fi
    i=0
    for file in "${profiles[@]}"; do
        if [ -L "$file" ]; then printf 'Kept linked shell profile: %s\n' "$file" >&2; continue; fi
        [ -e "$file" ] || continue
        if [ ! -f "$file" ] || ! profile_parent=$(plain_directory "$(dirname "$file")"); then
            printf 'Kept non-plain shell profile: %s\n' "$file" >&2; continue
        fi
        file="${profile_parent%/}/$(basename "$file")"
        LC_ALL=C grep -a -Fqx "$line" "$file" || continue
        cp "$file" "$work/profile.$i"
        LC_ALL=C tr -d '\000' < "$work/profile.$i" > "$work/profile-text.$i"
        if ! cmp -s "$work/profile.$i" "$work/profile-text.$i"; then
            printf 'Kept binary shell profile: %s\n' "$file" >&2; continue
        fi
        # Preserve every other byte, including an absent final newline. Shell
        # text is read as data rather than fed to sed/awk as executable syntax.
        {
            while IFS= read -r record; do
                [ "$record" = "$line" ] || printf '%s\n' "$record"
            done
            [ -z "$record" ] || [ "$record" = "$line" ] || printf '%s' "$record"
        } < "$work/profile.$i" > "$work/profile-new.$i"
        if [ "$dry_run" -eq 0 ]; then
            [ ! -L "$file" ] && cmp -s "$file" "$work/profile.$i" || { echo "Profile changed during uninstall: $file" >&2; exit 1; }
            cat "$work/profile-new.$i" > "$file"
        fi
        report_remove 'installer profile line' "$file"
        i=$((i + 1))
    done
fi
for old in "$prefix"/nagi-*; do
    stem=${old##*/}
    [[ "$stem" =~ ^nagi-([0-9]+\.[0-9]+\.[0-9]+)-$platform$ ]] || continue
    if [ -L "$old" ]; then printf 'Kept linked installation: %s\n' "$old" >&2; continue; fi
    [ -d "$old" ] || continue
    version=${BASH_REMATCH[1]}
    if snapshot "$old" > "$work/existing" &&
        fetch_release "$version" "$work/release-$version" > "$work/verify.log" 2>&1 &&
        snapshot "$work/release-$version/$stem" > "$work/expected" &&
        cmp -s "$work/expected" "$work/existing" &&
        snapshot "$old" > "$work/rechecked" && cmp -s "$work/existing" "$work/rechecked"; then
        if [ "$dry_run" -eq 1 ] || rm -rf "$old"; then report_remove distribution "$old"
        else printf 'Could not fully remove distribution: %s\n' "$old" >&2
        fi
    else printf 'Kept changed or unverifiable installation: %s\n' "$old" >&2
    fi
done
if [ "$dry_run" -eq 1 ]; then printf '[OK] Dry run complete; no installed files or profiles changed.\n'
else printf '[OK] Nagi uninstall complete. Any kept items are listed above.\n'; printf 'Restart your terminal and VS Code to refresh PATH.\n'
fi
