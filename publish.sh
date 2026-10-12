#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
repo='sonique6784/omacdiag'
repo_url="https://github.com/$repo"
marketplace='omacom/omarchy-plugin-marketplace'
plugin_id='io.github.sonique6784.omacdiag'
submit=0

case "${1:-}" in
  '') ;;
  --submit) submit=1 ;;
  *) echo 'Usage: ./publish.sh [--submit]' >&2; exit 2 ;;
esac
if (( $# > 1 )); then echo 'Usage: ./publish.sh [--submit]' >&2; exit 2; fi

for command_name in git gh jq python3 omarchy qmllint rg sha256sum; do
  command -v "$command_name" >/dev/null || { echo "Missing command: $command_name" >&2; exit 1; }
done

version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
manifest_version=$(jq -r '.version' manifest.json)
manifest_id=$(jq -r '.id' manifest.json)
[[ "$manifest_version" == "$version" ]] || { echo 'Manifest and Cargo versions differ' >&2; exit 1; }
[[ "$manifest_id" == "$plugin_id" ]] || { echo 'Unexpected plugin ID' >&2; exit 1; }

omarchy plugin validate .
qmllint -I "${OMARCHY_PATH:-/usr/share/omarchy}/shell" plugin/Launcher.qml
python3 -m unittest discover -s tests -p 'test_plugin.py'

if [[ -n $(git status --porcelain) ]]; then
  echo 'Commit all changes before publication.' >&2
  exit 1
fi
origin=$(git remote get-url origin)
case "$origin" in
  "git@github.com:$repo.git"|"https://github.com/$repo.git"|"https://github.com/$repo") ;;
  *) echo "Unexpected origin: $origin" >&2; exit 1 ;;
esac

branch=$(git branch --show-current)
[[ -n "$branch" ]] || { echo 'Check out a branch before publication.' >&2; exit 1; }
gh auth status >/dev/null
repo_info=$(gh repo view "$repo" --json visibility,defaultBranchRef)
visibility=$(jq -r '.visibility' <<<"$repo_info")
[[ "$visibility" == PUBLIC ]] || { echo 'The plugin repository must be public.' >&2; exit 1; }
default_branch=$(jq -r '.defaultBranchRef.name' <<<"$repo_info")
[[ "$branch" == "$default_branch" ]] || {
  echo "Merge the plugin into the repository default branch ($default_branch) before submission." >&2
  exit 1
}
remote_sha=$(git ls-remote origin "refs/heads/$branch" | cut -f1)
[[ "$remote_sha" == "$(git rev-parse HEAD)" ]] || {
  echo "Push the current commit to origin/$branch before publication." >&2
  exit 1
}

tag="v$version"
archive="omacdiag-$version-linux-x86_64-cli.tar.gz"
checksum="$archive.sha256"
record="docs/releases/$tag.md"
[[ -f "$record" ]] || { echo "Missing tested-release record: $record" >&2; exit 1; }
rg -q '^Status: passed$' "$record" || { echo "Release acceptance is not passed in $record" >&2; exit 1; }
tested_commit=$(sed -n 's/^Tested commit: //p' "$record")
[[ "$tested_commit" =~ ^[0-9a-f]{40}$ ]] || { echo 'Release record must contain one tested commit SHA.' >&2; exit 1; }
git merge-base --is-ancestor "$tested_commit" HEAD || { echo 'Tested commit is not an ancestor of the submission.' >&2; exit 1; }
unexpected_changes=$(git diff --name-only "$tested_commit" HEAD -- . ':!docs/releases' ':!docs/releases/**')
[[ -z "$unexpected_changes" ]] || {
  echo "Changes after the tested commit require a new archive and smoke test: $unexpected_changes" >&2
  exit 1
}
tag_commit=$(git ls-remote origin "refs/tags/$tag^{}" | cut -f1)
if [[ -z "$tag_commit" ]]; then
  tag_commit=$(git ls-remote origin "refs/tags/$tag" | cut -f1)
fi
[[ "$tag_commit" == "$tested_commit" ]] || {
  echo "The pushed $tag tag does not point to the tested commit in $record." >&2
  exit 1
}
release=$(gh release view "$tag" --repo "$repo" --json tagName,isDraft,isPrerelease,assets,url)
jq -e --arg tag "$tag" --arg archive "$archive" --arg checksum "$checksum" '
  .tagName == $tag and (.isDraft | not) and (.isPrerelease | not)
  and ([.assets[].name] | index($archive) != null and index($checksum) != null)
' <<<"$release" >/dev/null || {
  echo "Publish a final $tag release with $archive and $checksum first." >&2
  exit 1
}

rg -q '^Artifact SHA-256: [0-9a-f]{64}$' "$record" || {
  echo "Record the tested archive SHA-256 in $record" >&2
  exit 1
}
record_sha=$(sed -n 's/^Artifact SHA-256: //p' "$record")
[[ "$record_sha" =~ ^[0-9a-f]{64}$ ]] || { echo 'Release record must contain one SHA-256.' >&2; exit 1; }
release_dir=$(mktemp -d)
trap 'rm -rf -- "$release_dir"' EXIT
gh release download "$tag" --repo "$repo" --pattern "$checksum" --dir "$release_dir"
checksum_line=$(cat "$release_dir/$checksum")
[[ "$checksum_line" == "$record_sha  $archive" ]] || {
  echo "Release checksum must contain exactly the SHA-256 for $archive." >&2
  exit 1
}
gh release download "$tag" --repo "$repo" --pattern "$archive" --dir "$release_dir"
(cd "$release_dir" && sha256sum -c "$checksum")

existing=$(gh issue list --repo "$marketplace" --state all --limit 1000 \
  --search "$repo_url in:body" --json title,url,body \
  --jq '.[] | select(.title == "[Plugin]: OmacDiag" and (.body | contains("https://github.com/sonique6784/omacdiag"))) | .url')
[[ -z "$existing" ]] || { echo "Submission already exists: $existing" >&2; exit 1; }

body_file=$(mktemp)
trap 'rm -f "$body_file"; rm -rf -- "$release_dir"' EXIT
cat >"$body_file" <<EOF
### Repository URL

$repo_url

### Category

Hardware

### Tags

launcher, quickshell, system

### Suggest a missing tag

_No response_

### Maintainer notes

The plugin opens the OmacDiag diagnostic UI inside the existing Omarchy shell and runs the separately installed Rust CLI engine from /usr/bin/omacdiag. Users explicitly run scripts/menu_entry.py install after installing and enabling the plugin; removal removes only the owned menu row. The tested CLI release is $tag: $(jq -r '.url' <<<"$release"). The release acceptance record is in $record. The optional SMART helper needs a separate administrator installation and is not installed by the plugin.

### Submission checklist

- [x] The repository is public and contains installation and removal instructions.
- [x] I have documented the plugin license and any external dependencies.
- [x] I confirm that I own or have permission to submit this plugin and its preview assets.
- [x] The plugin does not overwrite user configuration without explicit consent.
- [x] I understand that approval is for listing and is not a security review.
EOF

echo 'Issue title: [Plugin]: OmacDiag'
cat "$body_file"
if (( ! submit )); then
  echo 'Dry run complete. Run ./publish.sh --submit to confirm and create the issue.'
  exit 0
fi

[[ -t 0 ]] || { echo 'Submission requires an interactive confirmation.' >&2; exit 1; }
read -r -p 'Confirm the five checklist statements and create this issue? Type submit: ' answer
[[ "$answer" == submit ]] || { echo 'Submission cancelled.' >&2; exit 1; }
gh issue create --repo "$marketplace" --title '[Plugin]: OmacDiag' --body-file "$body_file"
