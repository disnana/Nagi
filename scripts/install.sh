#!/usr/bin/env bash
# Install a verified release for the current user. No sudo or Rust installation.
set -euo pipefail
version=0.1.6
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
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Version must be X.Y.Z' >&2; exit 1; }
case "$(uname -s)/$(uname -m)" in
    Linux/x86_64) platform=linux-x86_64 ;;
    Darwin/arm64) platform=macos-arm64 ;;
    Darwin/x86_64) platform=macos-x86_64 ;;
    *) echo 'Supported: Linux x86_64, macOS arm64/x86_64. Use install.ps1 on Windows.' >&2; exit 1 ;;
esac
for tool in curl tar cmp diff; do command -v "$tool" >/dev/null || { echo "Required: $tool" >&2; exit 1; }; done
if command -v sha256sum >/dev/null; then
    hash() { sha256sum "$1" | cut -d ' ' -f 1; }
elif command -v shasum >/dev/null; then
    hash() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
else echo 'Required: sha256sum or shasum' >&2; exit 1
fi
mkdir -p "$prefix"
prefix=$(cd "$prefix" && pwd -P)
work=$(mktemp -d "$prefix/.install.XXXXXX")
trap 'rm -rf "$work"' EXIT
stem="nagi-$version-$platform"
asset="$stem.tar.gz"
base="https://github.com/disnana/Nagi/releases/download/nagi-v$version"
for file in "$asset" "$asset.sha256"; do
    curl --fail --location --proto '=https' --tlsv1.2 --retry 3 --connect-timeout 15 --max-time 300 "$base/$file" -o "$work/$file"
done
checksum=$(cat "$work/$asset.sha256")
read -r expected filename extra <<< "$checksum"
[[ "$expected" =~ ^[0-9a-f]{64}$ && "$filename" == "$asset" && -z "$extra" && "$checksum" != *$'\n'* ]] || { echo 'Invalid checksum file' >&2; exit 1; }
[ "$(hash "$work/$asset")" = "$expected" ] || { echo 'SHA-256 mismatch; nothing installed' >&2; exit 1; }
tar -tzf "$work/$asset" > "$work/entries"
while IFS= read -r entry; do
    case "$entry" in "$stem/"*) ;; *) echo 'Unexpected archive path' >&2; exit 1 ;; esac
    case "/$entry/" in *'/../'*|*'/./'*) echo 'Unsafe archive path' >&2; exit 1 ;; esac
done < "$work/entries"
tar -tvzf "$work/$asset" > "$work/modes"
while IFS= read -r entry; do
    case "$entry" in [-d]*) ;; *) echo 'Archive contains a link or special file' >&2; exit 1 ;; esac
done < "$work/modes"
tar -xzf "$work/$asset" --no-same-owner --no-same-permissions -C "$work"
root="$work/$stem"
[ -f "$root/runtime/Cargo.toml" ] && [ -f "$root/runtime/src/lib.rs" ] && [ -f "$root/release.json" ] || { echo 'Incomplete distribution' >&2; exit 1; }
grep -Fqx "  \"version\": \"$version\"," "$root/release.json" &&
    grep -Fqx "  \"platform\": \"$platform\"," "$root/release.json" || { echo 'Distribution metadata mismatch' >&2; exit 1; }
[ "$("$root/nagic" --version)" = "nagic $version" ] || { echo 'Compiler version mismatch' >&2; exit 1; }
destination="$prefix/$stem"
if [ -e "$destination" ]; then
    for file in nagic release.json LICENSE README.txt; do
        cmp -s "$root/$file" "$destination/$file" || { echo "Existing install differs: $destination (not overwritten)" >&2; exit 1; }
    done
    diff -qr "$root/runtime" "$destination/runtime" >/dev/null || { echo 'Existing runtime differs (not overwritten)' >&2; exit 1; }
else mv "$root" "$destination"
fi
mkdir -p "$bin_dir"
bin_dir=$(cd "$bin_dir" && pwd -P)
link="$bin_dir/nagic"
if [ -e "$link" ] || [ -L "$link" ]; then
    [ -L "$link" ] || { echo "Existing command not overwritten: $link" >&2; exit 1; }
    previous=$(readlink "$link")
    case "$previous" in "$prefix"/nagi-*/nagic) ;; *) echo "Existing command not overwritten: $link" >&2; exit 1 ;; esac
fi
ln -s "$destination/nagic" "$work/nagic-link"
mv -f "$work/nagic-link" "$link"
if [ "$no_path" -eq 0 ]; then
    # Quote a possibly custom path as shell data, never as executable syntax.
    printf -v quoted '%q' "$bin_dir"
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
        touch "$file"
        grep -Fqx "$line" "$file" || printf '\n%s\n' "$line" >> "$file"
    done
fi
"$link" --version
printf '%s\n' "Installed: $destination"
printf '%s\n' "For this terminal: export PATH=$(printf '%q' "$bin_dir"):\"\$PATH\""
printf '%s\n' 'Building applications requires Rust/Cargo and a C build environment. Restart VS Code to refresh PATH.'
if [ -n "${NAGI_ROOT:-}" ]; then printf '%s\n' 'NAGI_ROOT is set; unset it to use the installed runtime automatically.'; fi
