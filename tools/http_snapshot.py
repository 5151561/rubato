"""HTTP 录放快照的合成/录制库——key 算法的 Python 参考实现。

契约:docs/http-snapshot.md(§2-§5)。与
judge/harness/.../ReplayHttp.kt、rust/crates/difftest/src/replay.rs 逐字对齐;
改任何一处必须同步三处并重生成快照。
"""

import base64
import hashlib
import json
import os

WHITELIST = {"user-agent", "referer", "content-type", "x-requested-with"}


def normalize_url_for_key(url: str):
    """§3:返回 (规范化 URL, host 目录名)。输入应是发出的(okhttp 规范化)URL。"""
    scheme, sep, rest = url.partition("://")
    if not sep:
        scheme, rest = "", url
    scheme = scheme.lower()
    tail_idx = len(rest)
    for i, ch in enumerate(rest):
        if ch in "/?#":
            tail_idx = i
            break
    authority, tail = rest[:tail_idx], rest[tail_idx:]
    host_port = authority.rsplit("@", 1)[-1]  # userInfo 丢弃
    host, colon, port = host_port.rpartition(":")
    if colon and port.isdigit():
        pass
    else:
        host, port = host_port, None
    host = host.lower().rstrip(".")
    default_port = {"http": "80", "https": "443"}.get(scheme)
    if port == default_port:
        port = None

    path_query = tail.split("#", 1)[0]
    path, qmark, query = path_query.partition("?")
    if not path:
        path = "/"
    query = query if qmark and query else None
    if query is not None:
        segs = sorted(query.split("&"), key=lambda s: s.encode("utf-8"))
        query = "&".join(segs)

    out = f"{scheme}://{host}"
    host_dir = host
    if port is not None:
        out += f":{port}"
        host_dir = f"{host}_{port}"
    out += path
    if query is not None:
        out += f"?{query}"
    return out, host_dir


def snapshot_key(method: str, url: str, headers: dict, body: bytes | None):
    """§2:返回 (key, host 目录名, 规范化 URL)。headers 为将发出的请求头。"""
    normalized, host_dir = normalize_url_for_key(url)
    lines = [method.upper(), normalized]
    hashed = sorted(
        (k.lower(), v.strip())
        for k, v in headers.items()
        if k.lower() in WHITELIST
    )
    lines += [f"{k}: {v}" for k, v in hashed]
    lines.append(hashlib.sha256(body).hexdigest() if body is not None else "")
    digest = hashlib.sha256("\n".join(lines).encode("utf-8")).digest()
    return digest[:16].hex(), host_dir, normalized


def write_snapshot(root, *, method, url, headers, body=None,
                   status=200, response_headers=None, response_body=b"",
                   note="手工合成"):
    """按契约算 key 并落一份快照文件;返回 key。"""
    key, host_dir, _ = snapshot_key(method, url, headers, body)
    d = os.path.join(root, host_dir)
    os.makedirs(d, exist_ok=True)
    snap = {
        "key": key,
        "request": {
            "method": method.upper(),
            "url": url,
            "headers": dict(headers),
            "bodyBase64": base64.b64encode(body).decode() if body is not None else None,
        },
        "response": {
            "status": status,
            "headers": response_headers or {},
            "bodyBase64": base64.b64encode(response_body).decode(),
        },
        "recordedAt": "2026-08-29T00:00:00Z",
        "note": note,
    }
    with open(os.path.join(d, f"{key}.json"), "w", encoding="utf-8") as f:
        json.dump(snap, f, ensure_ascii=False, indent=1)
        f.write("\n")
    return key
