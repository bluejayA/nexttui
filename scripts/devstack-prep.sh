#!/usr/bin/env bash
# devstack-prep.sh — bring up + verify the DevStack VMs that nexttui
# targets, and make sure their Keystone catalogs are reachable from the
# macOS host.
#
# Two environments are supported (matching ~/.config/openstack/clouds.yaml):
#   - devstack       (single VM, all-in-one, shared IP 192.168.105.5)
#   - devstack-multi (multi-node, controller = devstack-ctrl, 192.168.105.2)
#
# Usage:
#   ./devstack-prep.sh             # start both environments + verify
#   ./devstack-prep.sh single      # start + verify just the all-in-one
#   ./devstack-prep.sh multi       # start + verify just the multi-node
#   ./devstack-prep.sh verify      # skip start, only verify reachability
#   ./devstack-prep.sh rewrite-single  # rewrite single VM catalog URLs
#   ./devstack-prep.sh rewrite-multi   # rewrite ctrl catalog URLs
#
# Requires:
#   - Lima (limactl) with socket_vmnet for the "shared" network
#   - python3 on PATH for the catalog-rewrite helper
#   - VMs pre-installed (this script does not run ./stack.sh)

set -euo pipefail

SINGLE_IP="192.168.105.5"
CTRL_IP="192.168.105.2"
SINGLE_VM="devstack"
CTRL_VM="devstack-ctrl"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

color() { printf '\033[%sm%s\033[0m\n' "$1" "$2"; }
info()  { color "36" "[info]  $*"; }
ok()    { color "32" "[ok]    $*"; }
warn()  { color "33" "[warn]  $*"; }
fail()  { color "31" "[fail]  $*"; }

ensure_started() {
  local vm="$1"
  local status
  status=$(limactl list "$vm" --format '{{.Status}}' 2>/dev/null || echo missing)
  case "$status" in
    Running) ok "$vm already running" ;;
    Stopped)
      info "starting $vm …"
      limactl start "$vm" >/dev/null
      ok "$vm started"
      ;;
    missing) fail "$vm VM not found — create it first"; return 1 ;;
    *) warn "$vm is in unexpected state: $status"; return 1 ;;
  esac
}

ensure_shared_ip() {
  local vm="$1"
  local want_ip="$2"
  local got
  got=$(limactl shell "$vm" -- ip -4 -br addr show lima0 2>/dev/null | awk '{print $3}' | cut -d/ -f1 || true)
  if [[ "$got" != "$want_ip" ]]; then
    info "$vm lima0 IP=${got:-none}, expected $want_ip — renewing DHCP"
    limactl shell "$vm" -- sudo netplan apply >/dev/null 2>&1 || true
    sleep 3
    got=$(limactl shell "$vm" -- ip -4 -br addr show lima0 2>/dev/null | awk '{print $3}' | cut -d/ -f1 || true)
  fi
  if [[ "$got" != "$want_ip" ]]; then
    fail "$vm never got $want_ip (has ${got:-none}) — check Lima shared network"
    return 1
  fi
  ok "$vm lima0 = $want_ip"
}

curl_status() {
  curl -s -o /dev/null -w '%{http_code}' -m 5 "$1"
}

verify_keystone() {
  local host="$1"
  local url="http://$host/identity/v3"
  local status
  status=$(curl_status "$url")
  if [[ "$status" == "200" ]]; then
    ok "$host keystone OK ($url → 200)"
  else
    fail "$host keystone unreachable ($url → $status)"
    return 1
  fi
}

verify_catalog_consistent() {
  local host="$1"
  local token
  token=$(curl -s -i -X POST "http://$host/identity/v3/auth/tokens" \
    -H 'Content-Type: application/json' \
    -d '{"auth":{"identity":{"methods":["password"],"password":{"user":{"name":"admin","password":"secret","domain":{"name":"Default"}}}},"scope":{"project":{"name":"admin","domain":{"name":"Default"}}}}}' \
    | awk 'tolower($1)=="x-subject-token:" {print $2; exit}' | tr -d '\r')
  if [[ -z "$token" ]]; then
    fail "$host: failed to get auth token"
    return 1
  fi
  local bad
  bad=$(curl -s "http://$host/identity/v3/endpoints" -H "X-Auth-Token: $token" \
    | python3 -c "
import json, sys, urllib.parse
data = json.load(sys.stdin)
bad = [e['url'] for e in data['endpoints']
       if urllib.parse.urlsplit(e['url']).hostname not in {sys.argv[1]}]
print('\n'.join(bad))
" "$host")
  if [[ -z "$bad" ]]; then
    ok "$host catalog: all endpoints use $host"
  else
    warn "$host catalog has stray URLs:"
    echo "$bad" | sed 's/^/       /'
    return 1
  fi
}

rewrite_catalog() {
  local host="$1"
  local stale="$2"
  info "rewriting $host catalog → hostname $host (stale hosts: $stale)"
  python3 "$SCRIPT_DIR/rewrite_catalog.py" \
    "http://$host/identity/v3" "$host" "$stale"
}

cmd_start_single() {
  ensure_started "$SINGLE_VM"
  ensure_shared_ip "$SINGLE_VM" "$SINGLE_IP"
  verify_keystone "$SINGLE_IP"
  verify_catalog_consistent "$SINGLE_IP" || {
    warn "run './devstack-prep.sh rewrite-single' to heal"
    return 1
  }
}

cmd_start_multi() {
  ensure_started "$CTRL_VM"
  ensure_shared_ip "$CTRL_VM" "$CTRL_IP"
  verify_keystone "$CTRL_IP"
  verify_catalog_consistent "$CTRL_IP" || {
    warn "run './devstack-prep.sh rewrite-multi' to heal"
    return 1
  }
}

cmd_verify() {
  verify_keystone "$SINGLE_IP" || true
  verify_keystone "$CTRL_IP" || true
  verify_catalog_consistent "$SINGLE_IP" || true
  verify_catalog_consistent "$CTRL_IP" || true
}

case "${1:-both}" in
  both)            cmd_start_single && cmd_start_multi ;;
  single)          cmd_start_single ;;
  multi)           cmd_start_multi ;;
  verify)          cmd_verify ;;
  rewrite-single)  rewrite_catalog "$SINGLE_IP" "localhost,192.168.5.15" ;;
  rewrite-multi)   rewrite_catalog "$CTRL_IP"   "localhost,192.168.5.15" ;;
  *) echo "unknown command: $1" >&2; exit 2 ;;
esac
