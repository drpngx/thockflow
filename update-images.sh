#!/bin/bash
set -euo pipefail

# Re-pin the base images in MODULE.bazel to what their tags currently point at, then
# fetch them.
#
# oci.pull entries that use `tag = "..."` float on their own and need nothing from
# this script. Entries that use `digest = "sha256:..."` are pinned and immutable --
# `bazel fetch` will happily re-fetch that exact digest forever. Bumping them is a
# source edit, which is what this does.
#
#   update-images.sh             re-pin, then bazel fetch
#   update-images.sh --check     report drift, change nothing (exit 1 if any)
#   update-images.sh --no-fetch  re-pin, skip the fetch
#
# After this, rebuild and restart to actually run the new image:
#   envgpg -e CF_TUNNEL_THOCKFLOW bazel run //server:serve

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODULE="$REPO/MODULE.bazel"

# oci.pull entries pinned by digest, as: <repo name>|<image>|<tag to track>
PINNED=(
    "cloudflared|docker.io/cloudflare/cloudflared|latest"
)

CHECK=0
FETCH=1
case "${1:-}" in
    --check)    CHECK=1 ;;
    --no-fetch) FETCH=0 ;;
    "")         ;;
    *) echo "usage: ${0##*/} [--check|--no-fetch]" >&2; exit 1 ;;
esac

# Resolve a tag to its manifest-list digest. That is what oci.pull pins: the index,
# not a per-platform manifest, so ask for the index media types explicitly.
resolve_digest() {
    local image="$1" tag="$2" repo token digest
    case "$image" in
        docker.io/*) repo="${image#docker.io/}" ;;
        *) echo "only docker.io images are supported here, got: $image" >&2; return 1 ;;
    esac

    token=$(curl -fsS "https://auth.docker.io/token?service=registry.docker.io&scope=repository:${repo}:pull" \
            | jq -r '.token')
    [[ -n "$token" && "$token" != "null" ]] || { echo "could not get a pull token for $repo" >&2; return 1; }

    digest=$(curl -fsSI \
        -H "Authorization: Bearer $token" \
        -H "Accept: application/vnd.oci.image.index.v1+json,application/vnd.docker.distribution.manifest.list.v2+json" \
        "https://registry-1.docker.io/v2/${repo}/manifests/${tag}" \
        | tr -d '\r' | awk -F': ' 'tolower($1)=="docker-content-digest"{print $2}')
    [[ "$digest" == sha256:* ]] || { echo "no digest in the registry response for ${repo}:${tag}" >&2; return 1; }
    printf '%s\n' "$digest"
}

# Replace the digest line inside the oci.pull block whose name matches.
repin() {
    local name="$1" digest="$2" tmp
    tmp=$(mktemp)
    awk -v name="$name" -v digest="$digest" '
        /^oci\.pull\(/            { inblock = 1; matched = 0 }
        inblock && $0 ~ "name = \"" name "\""  { matched = 1 }
        inblock && matched && /^[[:space:]]*digest = "sha256:/ {
            sub(/sha256:[0-9a-f]+/, digest); matched = 0
        }
        /^\)/                     { inblock = 0; matched = 0 }
                                  { print }
    ' "$MODULE" > "$tmp"
    mv "$tmp" "$MODULE"
}

DRIFTED=0
for entry in "${PINNED[@]}"; do
    IFS='|' read -r name image tag <<< "$entry"

    current=$(awk -v name="$name" '
        /^oci\.pull\(/ { inblock = 1; matched = 0 }
        inblock && $0 ~ "name = \"" name "\"" { matched = 1 }
        inblock && matched && /^[[:space:]]*digest = "sha256:/ {
            match($0, /sha256:[0-9a-f]+/); print substr($0, RSTART, RLENGTH); exit
        }
        /^\)/ { inblock = 0; matched = 0 }
    ' "$MODULE")

    if [[ -z "$current" ]]; then
        echo "! $name: no 'digest =' line found in $MODULE -- is it pinned by tag?" >&2
        exit 1
    fi

    latest=$(resolve_digest "$image" "$tag")

    if [[ "$current" == "$latest" ]]; then
        echo "  current  $name  ${current:0:19}…"
        continue
    fi

    DRIFTED=1
    echo "  BEHIND   $name  ${current:0:19}… -> ${latest:0:19}…"
    ((CHECK)) || repin "$name" "$latest"
done

if ((CHECK)); then
    ((DRIFTED)) && exit 1
    exit 0
fi

if ((DRIFTED == 0)); then
    echo
    echo "Nothing to re-pin."
    ((FETCH)) || exit 0
fi

if ((FETCH)); then
    echo
    echo "=== bazel fetch //... ==="
    # Default lockfile mode is 'update', so this also refreshes MODULE.bazel.lock.
    (cd "$REPO" && bazel fetch //...)
fi

if ((DRIFTED)); then
    echo
    echo "Re-pinned. Build and restart to actually run it:"
    echo "  envgpg -e CF_TUNNEL_THOCKFLOW bazel run //server:serve"
fi
