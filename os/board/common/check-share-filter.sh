#!/bin/sh
# Checks the packet filter of the network share in a target directory. The
# share is open to guests, so only clients on the local network may reach
# it (docs/spec/network-share.md, "Attack surface"). Exits with 0 when the
# image holds the filter, the tool that loads it and the init script that
# loads it, else with 1 and one line for each fault.
#
# Usage: check-share-filter.sh TARGET_DIR
#
# The rule lines of the filter must be exactly the ones below: a rule that
# is added, removed or changed in the image fails the build, also one that
# a later overlay brings.

target=$1
rules=$target/etc/neptron/share-filter.rules
script=$target/etc/init.d/S45neptron-share
status=0

expected='*filter
:INPUT ACCEPT [0:0]
:FORWARD ACCEPT [0:0]
:OUTPUT ACCEPT [0:0]
:neptron-lan - [0:0]
-A INPUT -p tcp -m tcp --dport 445 -j neptron-lan
-A INPUT -p udp -m udp --dport 5353 -j neptron-lan
-A neptron-lan -s 127.0.0.0/8 -j ACCEPT
-A neptron-lan -s 10.0.0.0/8 -j ACCEPT
-A neptron-lan -s 172.16.0.0/12 -j ACCEPT
-A neptron-lan -s 192.168.0.0/16 -j ACCEPT
-A neptron-lan -s 169.254.0.0/16 -j ACCEPT
-A neptron-lan -j DROP
COMMIT'

if [ ! -f "$rules" ]; then
	echo "check-share-filter: the image has no /etc/neptron/share-filter.rules; the network share would be open to every address" >&2
	status=1
elif [ "$(grep -v -e '^#' -e '^[[:space:]]*$' "$rules")" != "$expected" ]; then
	echo "check-share-filter: the rules of /etc/neptron/share-filter.rules are not the ones that limit the network share to the local network" >&2
	status=1
fi
if [ ! -e "$target/usr/sbin/iptables-legacy-restore" ] && [ ! -L "$target/usr/sbin/iptables-legacy-restore" ]; then
	echo "check-share-filter: the image has no /usr/sbin/iptables-legacy-restore to load the filter of the network share" >&2
	status=1
fi
if ! grep -qF 'if ! iptables-legacy-restore /etc/neptron/share-filter.rules; then' "$script" 2>/dev/null; then
	echo "check-share-filter: /etc/init.d/S45neptron-share does not load the filter before it starts the network share" >&2
	status=1
fi
if [ -e "$target/etc/init.d/S35iptables" ]; then
	echo "check-share-filter: the image has /etc/init.d/S35iptables, which flushes the filter at stop" >&2
	status=1
fi
exit $status
