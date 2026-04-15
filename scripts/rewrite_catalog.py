#!/usr/bin/env python3
"""Rewrite Keystone catalog URLs to use a consistent host.

Maps any URL whose host is in the 'from' set to the target host, preserving
the path. Used to clean up DevStack's mixed-host catalog after installing the
Lima shared network.
"""
import json
import sys
import urllib.parse
import urllib.request


def auth(auth_url: str, username: str, password: str, project: str) -> str:
    body = {
        "auth": {
            "identity": {
                "methods": ["password"],
                "password": {
                    "user": {
                        "name": username,
                        "password": password,
                        "domain": {"name": "Default"},
                    }
                },
            },
            "scope": {"project": {"name": project, "domain": {"name": "Default"}}},
        }
    }
    req = urllib.request.Request(
        f"{auth_url}/auth/tokens",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        return resp.headers["X-Subject-Token"]


def list_endpoints(auth_url: str, token: str) -> list:
    req = urllib.request.Request(
        f"{auth_url}/endpoints", headers={"X-Auth-Token": token}
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        return json.loads(resp.read())["endpoints"]


def patch_endpoint(auth_url: str, token: str, endpoint_id: str, new_url: str):
    body = json.dumps({"endpoint": {"url": new_url}}).encode()
    req = urllib.request.Request(
        f"{auth_url}/endpoints/{endpoint_id}",
        data=body,
        headers={"X-Auth-Token": token, "Content-Type": "application/json"},
        method="PATCH",
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        return json.loads(resp.read())


def rewrite_host(url: str, target_host: str, stale_hosts: set) -> str:
    """Replace the netloc in URL with target_host if it matches a stale one."""
    parsed = urllib.parse.urlsplit(url)
    host = parsed.hostname
    if host in stale_hosts:
        # Drop port — target_host already includes scheme/host only.
        new_netloc = target_host
        return urllib.parse.urlunsplit(
            (parsed.scheme, new_netloc, parsed.path, parsed.query, parsed.fragment)
        )
    return url


def main():
    if len(sys.argv) < 4:
        print(
            "usage: rewrite_catalog.py <keystone-v3-url> <target-host> "
            "<stale-host>[,<stale-host>...]",
            file=sys.stderr,
        )
        sys.exit(2)
    keystone = sys.argv[1].rstrip("/")
    target_host = sys.argv[2]
    stale_hosts = set(sys.argv[3].split(","))

    token = auth(keystone, "admin", "secret", "admin")
    endpoints = list_endpoints(keystone, token)
    print(f"Found {len(endpoints)} endpoints. Target netloc = {target_host}")
    print(f"Stale hosts to rewrite: {sorted(stale_hosts)}")
    print()
    changed = 0
    for ep in endpoints:
        old = ep["url"]
        new = rewrite_host(old, target_host, stale_hosts)
        if new != old:
            patch_endpoint(keystone, token, ep["id"], new)
            print(f"  [REWRITE] {ep['interface']:8} {ep['service_id'][:8]}  {old}")
            print(f"              → {new}")
            changed += 1
        else:
            print(f"  [keep]    {ep['interface']:8} {ep['service_id'][:8]}  {old}")
    print()
    print(f"Rewrote {changed}/{len(endpoints)} endpoints.")


if __name__ == "__main__":
    main()
