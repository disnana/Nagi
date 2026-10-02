#!/usr/bin/env bash
# Install a verified release for the current user. No sudo or Rust installation.
set -euo pipefail
version=latest
prefix="$HOME/.local/share/nagi"
bin_dir="$HOME/.local/bin"
no_path=0
profile=''
while [ "$#" -gt 0 ]; do
    case "$1" in
        --version) version=${2:?Missing version}; shift 2 ;;
        --prefix) prefix=${2:?Missing install directory}; shift 2 ;;
        --bin-dir) bin_dir=${2:?Missing bin directory}; shift 2 ;;
        --profile) profile=${2:?Missing shell profile}; shift 2 ;;
        --no-path) no_path=1; shift ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done
[[ "$version" == latest || "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Version must be latest or X.Y.Z' >&2; exit 1; }
case "$(uname -s)/$(uname -m)" in
    Linux/x86_64) platform=linux-x86_64 ;;
    Darwin/arm64) platform=macos-arm64 ;;
    Darwin/x86_64) platform=macos-x86_64 ;;
    *) echo 'Supported: Linux x86_64, macOS arm64/x86_64. Use install.ps1 on Windows.' >&2; exit 1 ;;
esac
for tool in curl tar cmp find sort; do command -v "$tool" >/dev/null || { echo "Required: $tool" >&2; exit 1; }; done
if command -v sha256sum >/dev/null; then
    hash() { sha256sum "$1" | cut -d ' ' -f 1; }
elif command -v shasum >/dev/null; then
    hash() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
else echo 'Required: sha256sum or shasum' >&2; exit 1
fi
mkdir -p "$prefix"
prefix=$(cd "$prefix" && pwd -P)
lock="$prefix/.install-lock"
mkdir "$lock" 2>/dev/null || { echo "Another installer is running, or a previous run was interrupted: $lock" >&2; exit 1; }
work=''
link_work=''
previous=''
activated=0
committed=0
changed_profiles=()
cleanup() {
    local status=$? i file
    trap - EXIT
    if [ "$committed" -eq 0 ]; then
        if [ "$activated" -eq 1 ]; then
            if [ -n "$previous" ]; then
                ln -s "$previous" "$link_work/restore"
                mv -f "$link_work/restore" "$link"
            else rm -f "$link"
            fi
        fi
        for ((i=0; i<${#changed_profiles[@]}; i++)); do
            file=${changed_profiles[$i]}
            if [ -f "$work/profile.$i" ]; then cat "$work/profile.$i" > "$file"
            else rm -f "$file"
            fi
        done
    fi
    [ -z "$link_work" ] || rm -rf "$link_work"
    [ -z "$work" ] || rm -rf "$work"
    rmdir "$lock"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
work=$(mktemp -d "$prefix/.install.XXXXXX")
download() {
    curl --fail --location --proto '=https' --proto-redir '=https' --tlsv1.2 --retry 3 --connect-timeout 15 --max-time 300 "$@"
}
if [ "$version" = latest ]; then
    # The release publisher marks only Nagi releases as Latest, never VSIX.
    latest=$(download --head --silent --show-error -o /dev/null --write-out '%{url_effective}' https://github.com/disnana/Nagi/releases/latest)
    case "$latest" in
        https://github.com/disnana/Nagi/releases/tag/nagi-v*) version=${latest##*/nagi-v} ;;
        *) echo 'Latest is not a Nagi release; use --version X.Y.Z' >&2; exit 1 ;;
    esac
    [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Latest is not a formal Nagi release' >&2; exit 1; }
fi

# Download into a new directory and validate before extracting or executing.
# Every operation must check its status: cleanup calls this inside an if.
fetch_release() {
    local release=$1 folder=$2 name="nagi-$1-$platform" file entry checksum expected filename extra
    local asset="$name.tar.gz" base="https://github.com/disnana/Nagi/releases/download/nagi-v$release"
    mkdir "$folder" || return 1
    for file in "$asset" "$asset.sha256"; do
        download "$base/$file" -o "$folder/$file" || return 1
    done
    checksum=$(cat "$folder/$asset.sha256") || return 1
    read -r expected filename extra <<< "$checksum"
    [[ "$expected" =~ ^[0-9a-f]{64}$ && "$filename" == "$asset" && -z "$extra" && "$checksum" != *$'\n'* ]] || { echo 'Invalid checksum file' >&2; return 1; }
    [ "$(hash "$folder/$asset")" = "$expected" ] || { echo 'SHA-256 mismatch; nothing installed' >&2; return 1; }
    tar -tzf "$folder/$asset" > "$folder/entries" || return 1
    while IFS= read -r entry; do
        [[ "$entry" == "$name/"* && "$entry" != *[^a-zA-Z0-9._/-]* ]] || { echo 'Unexpected archive path' >&2; return 1; }
        case "/$entry/" in *'/../'*|*'/./'*) echo 'Unsafe archive path' >&2; return 1 ;; esac
    done < "$folder/entries"
    tar -tvzf "$folder/$asset" > "$folder/modes" || return 1
    while IFS= read -r entry; do
        case "$entry" in [-d]*) ;; *) echo 'Archive contains a link or special file' >&2; return 1 ;; esac
    done < "$folder/modes"
    tar -xzf "$folder/$asset" --no-same-owner --no-same-permissions -C "$folder" || return 1
    for file in nagic runtime/Cargo.toml runtime/src/lib.rs release.json LICENSE README.txt; do
        [ -f "$folder/$name/$file" ] || { echo 'Incomplete distribution' >&2; return 1; }
    done
    grep -Fqx "  \"version\": \"$release\"," "$folder/$name/release.json" &&
        grep -Fqx "  \"platform\": \"$platform\"," "$folder/$name/release.json" || { echo 'Distribution metadata mismatch' >&2; return 1; }
}

# Compare the entire tree, including added/empty directories and hidden files.
# Links and unusual file names are preserved, never followed during cleanup.
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
stem="nagi-$version-$platform"
destination="$prefix/$stem"
fetch_release "$version" "$work/new"
root="$work/new/$stem"
snapshot "$root" > "$work/expected"
[ "$("$root/nagic" --version)" = "nagic $version" ] || { echo 'Compiler version mismatch' >&2; exit 1; }
if [ -e "$destination" ] || [ -L "$destination" ]; then
    snapshot "$destination" > "$work/existing" && cmp -s "$work/expected" "$work/existing" || { echo "Existing install differs: $destination (not overwritten)" >&2; exit 1; }
else mv "$root" "$destination"
fi
mkdir -p "$bin_dir"
bin_dir=$(cd "$bin_dir" && pwd -P)
link="$bin_dir/nagic"
if [ -e "$link" ] || [ -L "$link" ]; then
    [ -L "$link" ] || { echo "Existing command not overwritten: $link" >&2; exit 1; }
    previous=$(readlink "$link")
    old_stem=${previous#"$prefix/"}
    [[ "$previous" == "$prefix/"* && "$old_stem" =~ ^nagi-[0-9]+\.[0-9]+\.[0-9]+-$platform/nagic$ ]] || { echo "Existing command not overwritten: $link" >&2; exit 1; }
fi
link_work=$(mktemp -d "$bin_dir/.nagi-link.XXXXXX")
ln -s "$destination/nagic" "$link_work/new"
activated=1
mv -f "$link_work/new" "$link"
[ "$("$link" --version)" = "nagic $version" ] || { echo 'Activated compiler failed; restoring the previous version' >&2; exit 1; }
# Single quotes work in Bash 3.2 and zsh; keep replacement escapes outside double quotes.
quoted=${bin_dir//\'/\'\\\'\'}
quoted="'$quoted'"
if [ "$no_path" -eq 0 ]; then
    # Quote a possibly custom path as shell data, never as executable syntax.
    line="case \"\$PATH\" in $quoted|$quoted:*) ;; *) export PATH=$quoted:\"\$PATH\" ;; esac # Nagi installer"
    if [ -n "$profile" ]; then
        profiles=("$profile")
    elif [[ "${SHELL:-/bin/bash}" == */zsh ]]; then
        profiles=("$HOME/.zprofile" "$HOME/.zshrc")
    else
        login="$HOME/.profile"
        for candidate in "$HOME/.bash_profile" "$HOME/.bash_login"; do
            if [ -f "$candidate" ]; then login="$candidate"; break; fi
        done
        profiles=("$login" "$HOME/.bashrc")
    fi
    for file in "${profiles[@]}"; do
        [ ! -e "$file" ] || [ -f "$file" ] || { echo "Not a profile file: $file" >&2; exit 1; }
        if [ ! -f "$file" ] || ! grep -Fqx "$line" "$file"; then
            i=${#changed_profiles[@]}
            [ ! -f "$file" ] || cp "$file" "$work/profile.$i"
            changed_profiles[$i]=$file
            printf '\n%s\n' "$line" >> "$file"
        fi
    done
fi
committed=1
# A 0.1.6 install has no receipt. Verify older trees against their published
# archive before deleting them; changed/unverifiable trees stay untouched.
for old in "$prefix"/nagi-*; do
    [ "$old" != "$destination" ] && [ -d "$old" ] && [ ! -L "$old" ] || continue
    old_stem=${old##*/}
    [[ "$old_stem" =~ ^nagi-([0-9]+\.[0-9]+\.[0-9]+)-$platform$ ]] || continue
    old_version=${BASH_REMATCH[1]}
    if fetch_release "$old_version" "$work/old-$old_version" > "$work/cleanup.log" 2>&1 &&
        snapshot "$work/old-$old_version/$old_stem" > "$work/old-expected" &&
        snapshot "$old" > "$work/old-existing" && cmp -s "$work/old-expected" "$work/old-existing"; then
        if rm -rf "$old"; then printf 'Removed old version: %s\n' "$old"
        else printf 'Could not fully remove old version: %s\n' "$old" >&2
        fi
    else printf 'Kept changed or unverifiable old installation: %s\n' "$old" >&2
    fi
done
"$link" --version
printf '%s\n' "Installed: $destination"
printf '%s\n' "For this terminal: export PATH=$quoted:\"\$PATH\""
printf '%s\n' 'Building applications requires Rust/Cargo and a C build environment. Restart VS Code to refresh PATH.'
if [ -n "${NAGI_ROOT:-}" ]; then printf '%s\n' 'NAGI_ROOT is set; unset it to use the installed runtime automatically.'; fi
