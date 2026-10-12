#!/bin/bash
# Download the private resume archive into /workspace, verify its SHA-256 and print
# its RESTORE.md. Extraction is left to RESTORE.md's own steps.
#
# Source, in order of preference:
#   CF_RESUME_URL    an HTTPS URL, for example a presigned object-store link;
#   CF_RESUME_GH_REPO/CF_RESUME_GH_TAG
#                    a release asset in a private GitHub repository. Attach that
#                    repository to the session so the GitHub proxy authorizes it.
# CF_RESUME_ASSET names the file and CF_RESUME_SHA256 its expected digest.
set -euo pipefail
asset=${CF_RESUME_ASSET:?set CF_RESUME_ASSET}
want=${CF_RESUME_SHA256:?set CF_RESUME_SHA256}
[[ $want =~ ^[0-9a-f]{64}$ ]] || { echo "CF_RESUME_SHA256 is not a SHA-256 digest" >&2; exit 2; }
dest=/workspace/$asset
mkdir -p /workspace

if [ -f "$dest" ] && echo "$want  $dest" | sha256sum -c --status; then
  echo "verified $dest"
else
  rm -f "$dest.part"
  if [ -n "${CF_RESUME_URL:-}" ]; then
    curl -fL --retry 3 --proto =https -o "$dest.part" "$CF_RESUME_URL"
  else
    repo=${CF_RESUME_GH_REPO:?set CF_RESUME_URL or CF_RESUME_GH_REPO}
    tag=${CF_RESUME_GH_TAG:?set CF_RESUME_GH_TAG}
    id=$(gh api "repos/$repo/releases/tags/$tag" --jq ".assets[] | select(.name == \"$asset\") | .id")
    [ -n "$id" ] || { echo "no asset $asset in $repo release $tag" >&2; exit 1; }
    gh api -H 'Accept: application/octet-stream' "repos/$repo/releases/assets/$id" > "$dest.part"
  fi
  echo "$want  $dest.part" | sha256sum -c -
  mv "$dest.part" "$dest"
fi

restore=$(tar -tzf "$dest" | grep -m1 '\(^\|/\)RESTORE\.md$' || true)
[ -n "$restore" ] || { echo "no RESTORE.md in $dest" >&2; exit 1; }
echo "----- $restore -----"
tar -xzOf "$dest" "$restore"
