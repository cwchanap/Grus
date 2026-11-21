#!/usr/bin/env python3
"""HPA-697 evidence inspection via the chrome-devtools MCP (stdio transport).

Speaks the real MCP protocol: initialize -> tools/list -> new_page(file://) ->
resize_page -> take_screenshot, then saves the image the MCP returns.
Targets are committed outputs of the standard CLI smoke journeys; the MCP only
views local files -- it never launches the game or writes to the project.
"""
import base64, json, os, subprocess, sys, threading, queue, time

CHROME = "/home/opc/.cache/ms-playwright/chromium-1248/chrome-linux-arm64/chrome"
REPO = "/home/opc/orca/workspaces/Grus/hpa-697-painted-building-sprites-in-play-design"
EVID = os.path.join(REPO, "docs/superpowers/evidence/hpa-697-step4")
OUTDIR = os.path.join(EVID, "mcp-captures")
os.makedirs(OUTDIR, exist_ok=True)

TARGETS = [  # (committed file, natural width, height)
    ("mcp-inspection-sheet.png", 3856, 784),
    ("economy-cold.png", 1280, 720),
    ("economy-warm.png", 1280, 720),
    ("overlap-front.png", 1280, 720),
    ("overlap-behind.png", 1280, 720),
]

proc = subprocess.Popen(
    ["npx", "-y", "chrome-devtools-mcp@latest", "--no-usage-statistics",
     "--headless", "--isolated", "--executablePath", CHROME],
    cwd=OUTDIR, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
    stderr=subprocess.PIPE, text=True, bufsize=1)
errlines = []
def err_reader():
    for line in proc.stderr:
        errlines.append(line.rstrip())
threading.Thread(target=err_reader, daemon=True).start()
out_q = queue.Queue()
def out_reader():
    for line in proc.stdout:
        out_q.put(line.strip())
    out_q.put(None)
threading.Thread(target=out_reader, daemon=True).start()

_id = [0]
def send(method, params=None, notify=False):
    m = {"jsonrpc": "2.0", "method": method}
    if not notify:
        _id[0] += 1
        m["id"] = _id[0]
    if params is not None:
        m["params"] = params
    proc.stdin.write(json.dumps(m) + "\n")
    proc.stdin.flush()
    return m.get("id")

def recv(want_id, timeout=120):
    end = time.time() + timeout
    while time.time() < end:
        try:
            line = out_q.get(timeout=2)
        except queue.Empty:
            continue
        if line is None:
            raise RuntimeError("MCP stdout closed; stderr tail: " + " | ".join(errlines[-5:]))
        try:
            m = json.loads(line)
        except json.JSONDecodeError:
            continue
        if m.get("id") == want_id:
            if "error" in m:
                raise RuntimeError(f"MCP error for {want_id}: {json.dumps(m['error'])[:400]}")
            return m["result"]
    raise RuntimeError(f"timeout waiting for id={want_id}")

def call(name, args, timeout=120):
    rid = send("tools/call", {"name": name, "arguments": args})
    res = recv(rid, timeout)
    if res.get("isError"):
        raise RuntimeError(f"{name} isError: {json.dumps(res)[:400]}")
    return res

# --- handshake ---
rid = send("initialize", {"protocolVersion": "2025-03-26", "capabilities": {},
                          "clientInfo": {"name": "hpa697-mcp-inspection", "version": "1.0.0"}})
init = recv(rid)
print("INIT:", init["serverInfo"]["name"], init["serverInfo"]["version"], flush=True)
send("notifications/initialized", notify=True)
tools = recv(send("tools/list"), 60)["tools"]
assert any(t["name"] == "take_screenshot" for t in tools), "take_screenshot missing"
print("TOOLS:", len(tools), flush=True)

# --- per target: open -> resize -> screenshot -> save returned image ---
saved = []
for fname, w, h in TARGETS:
    src = os.path.join(EVID, fname)
    res = call("new_page", {"url": "file://" + src, "timeout": 30000})
    texts = [c.get("text", "") for c in res.get("content", []) if c.get("type") == "text"]
    page_id = None
    for c in res.get("content", []):
        sc = c.get("structuredContent")
        if isinstance(sc, dict) and "pageId" in sc:
            page_id = sc["pageId"]
    if page_id is None:
        import re
        for t in texts:
            m = re.search(r"^(\d+): .*\[selected\]", t, re.M)
            if m:
                page_id = int(m.group(1))
                break
    assert page_id is not None, f"no pageId for {fname}: {texts[:2]}"
    try:
        call("resize_page", {"pageId": page_id, "width": w, "height": h})
    except RuntimeError as e:
        print(f"  resize_page failed (continuing with fullPage): {e}", flush=True)
    out = os.path.join(OUTDIR, fname.replace(".png", "-via-mcp.png"))
    # v1.13: first screenshot may write to its own temp dir (path in response text);
    # later calls may return inline image content. Handle both.
    shot = call("take_screenshot", {"pageId": page_id, "format": "png",
                                    "fullPage": True}, 180)
    img = None
    for c in shot.get("content", []):
        if c.get("type") == "image" and c.get("data"):
            img = c
            break
    if img is not None:
        with open(out, "wb") as f:
            f.write(base64.b64decode(img["data"]))
        src_note = "inline-image-content"
    else:
        texts = [c.get("text", "") for c in shot.get("content", []) if c.get("type") == "text"]
        import re
        m = re.search(r"Saved screenshot to (\S+\.png)", "\n".join(texts))
        assert m, f"no image and no saved-path for {fname}: {texts[:2]}"
        import shutil
        shutil.copyfile(m.group(1), out)
        src_note = f"mcp_tmp={m.group(1)}"
    raw = os.path.getsize(out)
    assert raw > 0, f"empty MCP artifact for {fname}"
    saved.append((fname, out, raw))
    print(f"SAVED {out} bytes={raw} pageId={page_id} {src_note}", flush=True)

print("DONE", len(saved), "captures", flush=True)
proc.terminate()
