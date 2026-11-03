"""Resolve a project's named ACP endpoint set from its machine-local registry."""

from __future__ import annotations

import ipaddress
import json
import os
import platform
import re
import stat
import tempfile
from pathlib import Path
from urllib.parse import urlsplit


FORMAT_VERSION = 1
MAX_REGISTRY_BYTES = 65536
SET_NAME = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
ENDPOINT_NAME = re.compile(r"^[a-z0-9][a-z0-9_-]{0,31}$")
KEYCHAIN_ACCOUNT = re.compile(r"^ACP_GATEWAY_TOKEN(?:_[A-Z0-9_]+)?$")
STATE_AUTHORITY_ID = re.compile(r"^[a-z0-9][a-z0-9._:-]{0,127}$")


class EndpointRegistryError(ValueError):
    """The selected endpoint set is absent or invalid."""


def default_registry_path() -> Path:
    if platform.system() == "Darwin":
        return Path.home() / "Library" / "Application Support" / "formal-project-bootstrap" / "acp-endpoints.json"
    config_home = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")).expanduser()
    if not config_home.is_absolute():
        raise EndpointRegistryError("XDG_CONFIG_HOME must be an absolute path")
    return config_home / "formal-project-bootstrap" / "acp-endpoints.json"


def registry_path(env: dict[str, str]) -> Path:
    configured = env.get("ACP_ENDPOINT_REGISTRY", "").strip()
    path = Path(configured).expanduser() if configured else default_registry_path()
    if not path.is_absolute():
        raise EndpointRegistryError("ACP_ENDPOINT_REGISTRY must be an absolute path")
    return path


def _loopback(host: str) -> bool:
    if host.lower() == "localhost":
        return True
    try:
        return ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


def _validate_endpoint(value: object, *, source: str) -> dict:
    if not isinstance(value, dict) or set(value) - {
            "name", "url", "keychainAccount", "localOnly", "stateAuthorityId"}:
        raise EndpointRegistryError(f"{source} endpoint has an invalid shape")
    name, url = value.get("name"), value.get("url")
    account = value.get("keychainAccount")
    local_only = value.get("localOnly", False)
    state_authority_id = value.get("stateAuthorityId")
    if (not isinstance(name, str) or not ENDPOINT_NAME.fullmatch(name)
            or not isinstance(account, str) or not KEYCHAIN_ACCOUNT.fullmatch(account)
            or type(local_only) is not bool
            or not isinstance(url, str) or not url or len(url) > 2048 or any(char.isspace() for char in url)):
        raise EndpointRegistryError(f"{source} endpoint identity or keychain binding is invalid")
    try:
        parsed = urlsplit(url)
        host = parsed.hostname or ""
        parsed.port
    except ValueError as exc:
        raise EndpointRegistryError(f"{source} endpoint URL is invalid") from exc
    loopback = _loopback(host)
    if (not parsed.netloc or not host or parsed.username or parsed.password or parsed.query or parsed.fragment
            or parsed.path not in ("", "/")
            or parsed.scheme not in ("https", "http")
            or (parsed.scheme != "https" and not loopback)
            or (local_only and not loopback)
            or (loopback and not local_only)):
        raise EndpointRegistryError(f"{source} endpoint URL must be HTTPS; loopback endpoints must be explicitly localOnly")
    if state_authority_id is not None and (not isinstance(state_authority_id, str)
                                            or not STATE_AUTHORITY_ID.fullmatch(state_authority_id)):
        raise EndpointRegistryError(f"{source} endpoint stateAuthorityId is invalid")
    return {"name": name, "url": url.rstrip("/"), "keychain_account": account,
            "local_only": local_only, "state_authority_id": state_authority_id}


def _validate_set(name: str, entries: object, *, source: str) -> list[dict]:
    if not SET_NAME.fullmatch(name):
        raise EndpointRegistryError("authorizationProvider.gatewayEndpointSet is invalid")
    if not isinstance(entries, list) or not 1 <= len(entries) <= 3:
        raise EndpointRegistryError(f"{source} must contain one to three ordered endpoints")
    endpoints = [_validate_endpoint(item, source=source) for item in entries]
    names = [item["name"] for item in endpoints]
    urls = [item["url"].lower() for item in endpoints]
    accounts = [item["keychain_account"] for item in endpoints]
    if len(set(names)) != len(names) or len(set(urls)) != len(urls) or len(set(accounts)) != len(accounts):
        raise EndpointRegistryError(f"{source} endpoint names, URLs, and keychain accounts must be unique")
    if any(item["local_only"] for item in endpoints) and not all(item["local_only"] for item in endpoints):
        raise EndpointRegistryError("a named endpoint set cannot mix loopback and remote ACP endpoints")
    authority_ids = [item["state_authority_id"] for item in endpoints]
    if len(endpoints) > 1 and (not all(authority_ids) or len(set(authority_ids)) != 1):
        raise EndpointRegistryError("multi-endpoint sets require the same explicit stateAuthorityId on every endpoint")
    return endpoints


def _require_url(env: dict[str, str], url_env: str) -> None:
    if not env.get(url_env, "").strip():
        raise EndpointRegistryError(
            "no ACP gateway URL is configured: set authorizationProvider.gatewayEndpointSet in "
            f"PROJECT_PROFILE.json to use the machine endpoint registry, or set {url_env}")


def _legacy_endpoint(name: object, url: object, account: object, local_only: object = False) -> dict:
    if isinstance(url, str):
        try:
            parsed = urlsplit(url)
            local_only = local_only or _loopback(parsed.hostname or "")
        except ValueError:
            pass
    return {"name": name, "url": url, "keychainAccount": account, "localOnly": local_only}


def _read_registry(path: Path) -> dict:
    if path.is_symlink() or not path.is_file():
        raise EndpointRegistryError(f"machine ACP endpoint registry is missing or unsafe: {path}")
    metadata = path.stat()
    if metadata.st_size > MAX_REGISTRY_BYTES:
        raise EndpointRegistryError("machine ACP endpoint registry exceeds 64 KiB")
    if stat.S_IMODE(metadata.st_mode) & 0o077:
        raise EndpointRegistryError("machine ACP endpoint registry must be private (mode 0600)")
    def unique_object(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise EndpointRegistryError("machine ACP endpoint registry contains a duplicate JSON key")
            result[key] = value
        return result
    try:
        registry = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise EndpointRegistryError("machine ACP endpoint registry is unreadable or invalid JSON") from exc
    if (not isinstance(registry, dict) or set(registry) != {"formatVersion", "endpointSets"}
            or registry.get("formatVersion") != FORMAT_VERSION
            or type(registry.get("formatVersion")) is not int
            or not isinstance(registry.get("endpointSets"), dict)
            or not registry["endpointSets"]):
        raise EndpointRegistryError("machine ACP endpoint registry must use formatVersion 1 and endpointSets")
    return registry


def read_registry(path: Path) -> dict:
    registry = _read_registry(path)
    if len(registry["endpointSets"]) > 16:
        raise EndpointRegistryError("machine ACP endpoint registry contains more than 16 sets")
    for name, entries in registry["endpointSets"].items():
        if not isinstance(name, str):
            raise EndpointRegistryError("machine ACP endpoint registry contains an invalid set name")
        _validate_set(name, entries, source=f"endpoint set {name!r}")
    return registry


def configure_registry(path: Path, set_name: str, endpoint_values: list[dict], *,
                       shared_state_id: str | None = None, replace_set: bool = False) -> dict:
    """Atomically write URL and Keychain-account bindings, never bearer values."""
    if shared_state_id is not None:
        if not isinstance(shared_state_id, str) or not STATE_AUTHORITY_ID.fullmatch(shared_state_id):
            raise EndpointRegistryError("--shared-state-id must be a lowercase stable identifier")
        endpoint_values = [{**item, "stateAuthorityId": shared_state_id} for item in endpoint_values]
    endpoints = _validate_set(set_name, endpoint_values, source=f"endpoint set {set_name!r}")
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    registry = {"formatVersion": FORMAT_VERSION, "endpointSets": {}}
    if path.exists() or path.is_symlink():
        current = read_registry(path)
        registry["endpointSets"].update(current["endpointSets"])
    if set_name in registry["endpointSets"] and not replace_set:
        raise EndpointRegistryError(f"endpoint set {set_name!r} already exists; pass --replace-set to replace it")
    registry["endpointSets"][set_name] = [
        {"name": item["name"], "url": item["url"],
         "keychainAccount": item["keychain_account"], "localOnly": item["local_only"]}
         | ({"stateAuthorityId": item["state_authority_id"]} if item["state_authority_id"] else {})
        for item in endpoints
    ]
    encoded = (json.dumps(registry, indent=2, sort_keys=True) + "\n").encode("utf-8")
    if len(encoded) > MAX_REGISTRY_BYTES:
        raise EndpointRegistryError("machine ACP endpoint registry exceeds 64 KiB")
    descriptor, temporary_name = tempfile.mkstemp(prefix=".acp-endpoints-", dir=path.parent)
    temporary = Path(temporary_name)
    descriptor_open = True
    try:
        os.fchmod(descriptor, 0o600)
        stream = os.fdopen(descriptor, "wb")
        descriptor_open = False
        with stream:
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    except BaseException:
        if descriptor_open:
            os.close(descriptor)
        try:
            temporary.unlink()
        except OSError:
            pass
        raise
    read_registry(path)
    return registry


def resolve(acp: dict, env: dict[str, str]) -> tuple[str, list[dict]]:
    """Return the selected named set, normalizing a legacy single-URL profile once."""
    if "gatewayEndpointSet" in acp:
        set_name = acp.get("gatewayEndpointSet")
        if not isinstance(set_name, str) or not SET_NAME.fullmatch(set_name):
            raise EndpointRegistryError("authorizationProvider.gatewayEndpointSet is invalid")
        path = registry_path(env)
        registry = read_registry(path)
        entries = registry["endpointSets"].get(set_name)
        if entries is None:
            raise EndpointRegistryError(f"machine ACP endpoint registry has no set named {set_name!r}")
        return set_name, _validate_set(set_name, entries, source=f"endpoint set {set_name!r}")

    endpoint_specs = acp.get("gatewayEndpoints")
    if endpoint_specs is not None:
        if not isinstance(endpoint_specs, list) or len(endpoint_specs) != 1 or not isinstance(endpoint_specs[0], dict):
            raise EndpointRegistryError("legacy multi-endpoint profiles must select a named machine endpoint set")
        legacy = endpoint_specs[0]
        name = legacy.get("name", "legacy-primary")
        url_env = legacy.get("urlEnv", acp.get("gatewayUrlEnv", "ACP_GATEWAY_URL"))
        token_account = legacy.get("tokenEnv", acp.get("gatewayTokenEnv", "ACP_GATEWAY_TOKEN"))
        local_only = legacy.get("localOnly", False)
        if not isinstance(url_env, str) or not re.fullmatch(r"ACP_[A-Z0-9_]+", url_env):
            raise EndpointRegistryError("legacy ACP URL environment binding is invalid")
        _require_url(env, url_env)
        candidate = _legacy_endpoint(name, env.get(url_env, ""), token_account, local_only)
        return "legacy", _validate_set("legacy", [candidate], source="legacy ACP endpoint")

    url_env = acp.get("gatewayUrlEnv", "ACP_GATEWAY_URL")
    token_account = acp.get("gatewayTokenEnv", "ACP_GATEWAY_TOKEN")
    if not isinstance(url_env, str) or not re.fullmatch(r"ACP_[A-Z0-9_]+", url_env):
        raise EndpointRegistryError("legacy ACP URL environment binding is invalid")
    _require_url(env, url_env)
    candidate = _legacy_endpoint("legacy-primary", env.get(url_env, ""), token_account)
    return "legacy", _validate_set("legacy", [candidate], source="legacy ACP endpoint")
