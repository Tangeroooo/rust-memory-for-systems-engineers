#!/bin/sh
set -eu

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

git config --local user.name Tangeroooo
git config --local user.email juhyeon113@gmail.com
git config --local core.hooksPath .githooks

./scripts/check-git-identity.sh commit
printf '%s\n' 'repo-local identity and Git hooks are configured.'
