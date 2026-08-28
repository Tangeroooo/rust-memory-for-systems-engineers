#!/bin/sh
set -eu

expected_name='Tangeroooo'
expected_email='juhyeon113@gmail.com'
expected_repo='rust-memory-for-systems-engineers'

fail() {
    printf '%s\n' "identity guard: $*" >&2
    exit 1
}

check_ident() {
    label=$1
    ident=$2
    name=$(printf '%s\n' "$ident" | sed -E 's/^(.*) <[^>]+>.*$/\1/')
    email=$(printf '%s\n' "$ident" | sed -E 's/^.* <([^>]+)>.*$/\1/')
    [ "$name" = "$expected_name" ] || fail "$label name is '$name', expected '$expected_name'"
    [ "$email" = "$expected_email" ] || fail "$label email is '$email', expected '$expected_email'"
}

local_name=$(git config --local --get user.name 2>/dev/null || true)
local_email=$(git config --local --get user.email 2>/dev/null || true)
[ "$local_name" = "$expected_name" ] || fail "repo-local user.name is '$local_name'"
[ "$local_email" = "$expected_email" ] || fail "repo-local user.email is '$local_email'"

check_ident author "$(git var GIT_AUTHOR_IDENT)"
check_ident committer "$(git var GIT_COMMITTER_IDENT)"

mode=${1:-commit}
[ "$mode" = 'push' ] || exit 0

remote_name=${2:-origin}
remote_url=${3:-$(git remote get-url "$remote_name" 2>/dev/null || true)}
case "$remote_url" in
    git@github.com:Tangeroooo/$expected_repo.git|https://github.com/Tangeroooo/$expected_repo.git|https://github.com/Tangeroooo/$expected_repo)
        ;;
    *)
        fail "remote '$remote_name' is '$remote_url'; expected Tangeroooo/$expected_repo"
        ;;
esac

command -v gh >/dev/null 2>&1 || fail "gh CLI is required before push"
login=$(gh api user --jq .login 2>/dev/null || true)
[ "$login" = "$expected_name" ] || fail "active GitHub login is '${login:-unavailable}', expected '$expected_name'"

zero='0000000000000000000000000000000000000000'
while read -r local_ref local_sha remote_ref remote_sha; do
    [ -n "${local_sha:-}" ] || continue
    [ "$local_sha" = "$zero" ] && continue

    if [ "$remote_sha" = "$zero" ]; then
        commits=$(git rev-list "$local_sha")
    else
        commits=$(git rev-list "$remote_sha..$local_sha")
    fi

    for commit in $commits; do
        author_name=$(git show -s --format='%an' "$commit")
        author_email=$(git show -s --format='%ae' "$commit")
        committer_name=$(git show -s --format='%cn' "$commit")
        committer_email=$(git show -s --format='%ce' "$commit")

        [ "$author_name" = "$expected_name" ] || fail "$commit author name is '$author_name'"
        [ "$author_email" = "$expected_email" ] || fail "$commit author email is '$author_email'"
        [ "$committer_name" = "$expected_name" ] || fail "$commit committer name is '$committer_name'"
        [ "$committer_email" = "$expected_email" ] || fail "$commit committer email is '$committer_email'"
    done
done
