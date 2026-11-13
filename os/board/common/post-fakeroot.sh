#!/bin/sh
# Post-fakeroot step of both images. Buildroot calls it with the target
# directory as $1, after it makes the users of users.table, which the
# post-build step does not see yet.
#
# The share account neptron has a published SMB password. It must never
# allow a system login: no password hash in /etc/shadow, and no shell.

set -eu

target=$1
user=neptron

fail() {
	echo "post-fakeroot: $1" >&2
	exit 1
}

entry=$(grep "^$user:" "$target/etc/passwd" || true)
[ -n "$entry" ] || fail "no user $user in /etc/passwd for the network share"
shell=${entry##*:}
case $shell in
/bin/false | /sbin/nologin | /usr/sbin/nologin) ;;
*) fail "the user $user has a login shell ($shell)" ;;
esac

entry=$(grep "^$user:" "$target/etc/shadow" || true)
[ -n "$entry" ] || fail "no shadow entry for the user $user"
hash=$(echo "$entry" | cut -d: -f2)
case $hash in
"*" | "!"*) ;;
*) fail "the user $user has a password in /etc/shadow; it must allow no login" ;;
esac
