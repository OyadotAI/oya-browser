#!/usr/bin/env python3
"""
Regenerate crates/v9/src/v9/platform/chrome_shapes.txt, the table of every window interface
Chrome exposes with its members, from Blink's exposed-interface listings and IDL files.

Inputs: the stable listing (third_party/blink/web_tests/virtual/stable/webexposed/
global-interface-listing-expected.txt) and the platform-specific one
(third_party/blink/web_tests/webexposed/global-interface-listing-platform-specific-expected.txt)
from a Chromium checkout, and the IDL files under third_party/blink/renderer for constant
values (the pinned sparse clone at ~/Dev/upstream/chromium has them). Usage:

    python3 tools/chrome_shapes.py <stable-listing> <platform-listing> <out.txt>

Then prepend the header comment the existing table carries. Constants on interfaces whose IDL
files are outside the sparse clone come from the FALLBACK table below.
"""
import re, sys, glob, os
listing_paths = sys.argv[1:-1]; out = sys.argv[-1]
IDL_ROOT = os.path.expanduser("~/Dev/upstream/chromium/third_party/blink/renderer")

def parse_listing(path):
    ifaces = {}; cur = None
    for line in open(path):
        line = line.rstrip("\n")
        m = re.match(r"^interface (\w+)(?: : (\w+))?$", line)
        if m:
            cur = {"name": m.group(1), "parent": m.group(2) or "", "members": []}
            ifaces[m.group(1)] = cur; continue
        if line.startswith("[") or line.startswith("namespace"):
            cur = None; continue
        if cur is None or not line.startswith("    "): continue
        cur["members"].append(line.strip())
    return ifaces

ifaces = {}
for p in listing_paths:
    for k, v in parse_listing(p).items():
        ifaces.setdefault(k, v)

# constants from IDL: interface blocks (including partial) -> {NAME: value}
consts = {}
for path in glob.glob(IDL_ROOT + "/**/*.idl", recursive=True):
    text = open(path, errors="replace").read()
    text = re.sub(r"//.*", "", text)
    for m in re.finditer(r"(?:partial\s+)?interface\s+(?:mixin\s+)?(\w+)[^{;]*\{", text):
        name = m.group(1); depth = 0; i = m.end() - 1; start = i
        while i < len(text):
            if text[i] == "{": depth += 1
            elif text[i] == "}":
                depth -= 1
                if depth == 0: break
            i += 1
        body = text[start:i]
        for c in re.finditer(r"const\s+[\w\s]+?\s(\w+)\s*=\s*([\w.+-]+)\s*;", body):
            consts.setdefault(name, {})[c.group(1)] = c.group(2)
# mixins included by interfaces carry constants too (rare); resolve `X includes Y`
includes = {}
for path in glob.glob(IDL_ROOT + "/**/*.idl", recursive=True):
    for m in re.finditer(r"^\s*(\w+)\s+includes\s+(\w+)\s*;", open(path, errors="replace").read(), re.M):
        includes.setdefault(m.group(1), []).append(m.group(2))

FALLBACK = {"FileReader": {"EMPTY": "0", "LOADING": "1", "DONE": "2"},
    "XMLHttpRequest": {"UNSENT": "0", "OPENED": "1", "HEADERS_RECEIVED": "2", "LOADING": "3", "DONE": "4"},
    "XPathResult": {"ANY_TYPE": "0", "NUMBER_TYPE": "1", "STRING_TYPE": "2", "BOOLEAN_TYPE": "3", "UNORDERED_NODE_ITERATOR_TYPE": "4", "ORDERED_NODE_ITERATOR_TYPE": "5", "UNORDERED_NODE_SNAPSHOT_TYPE": "6", "ORDERED_NODE_SNAPSHOT_TYPE": "7", "ANY_UNORDERED_NODE_TYPE": "8", "FIRST_ORDERED_NODE_TYPE": "9"},
    "PerformanceNavigation": {"TYPE_NAVIGATE": "0", "TYPE_RELOAD": "1", "TYPE_BACK_FORWARD": "2", "TYPE_RESERVED": "255"},
    "GeolocationPositionError": {"PERMISSION_DENIED": "1", "POSITION_UNAVAILABLE": "2", "TIMEOUT": "3"},
    "Float16Array": {"BYTES_PER_ELEMENT": "2"}}
def const_value(iface, name):
    if name in FALLBACK.get(iface, {}): return FALLBACK[iface][name]
    chain = [iface] + includes.get(iface, [])
    for c in chain:
        if name in consts.get(c, {}): return consts[c][name]
    return None

lines = []; missing_consts = []
for name in sorted(ifaces):
    it = ifaces[name]; getters = set(); setters = set(); tokens = []
    for mem in it["members"]:
        parts = mem.split()
        static = parts[0] == "static"
        if static: parts = parts[1:]
        kind, member = parts[0], parts[1]
        if member == "constructor" and kind == "method": continue
        if member.startswith("@@"):
            if member == "@@toStringTag": continue
            if member == "@@iterator": tokens.append("i")
            elif member == "@@asyncIterator": tokens.append("y")
            continue
        if kind == "attribute":
            v = const_value(name, member)
            if v is None: missing_consts.append(f"{name}.{member}"); continue
            v = str(int(v, 16)) if v.lower().startswith("0x") else v
            tokens.append(f"c:{member}={v}")
        elif kind == "method": tokens.append(("M:" if static else "m:") + member)
        elif kind == "getter": (getters.add(("S" if static else "") + member))
        elif kind == "setter": (setters.add(("S" if static else "") + member))
    for g in sorted(getters):
        st = g.startswith("S"); n = g[1:] if st else g
        if g in setters: tokens.append(("A:" if st else "a:") + n)
        else: tokens.append(("G:" if st else "g:") + n)
    for s in sorted(setters - getters):
        st = s.startswith("S"); n = s[1:] if st else s
        tokens.append(("S:" if st else "s:") + n)
    lines.append(f"{name}|{it['parent']}|{' '.join(tokens)}")
open(out, "w").write("\n".join(lines) + "\n")
print(f"interfaces={len(lines)} consts_unresolved={len(missing_consts)}")
print("\n".join(missing_consts[:40]))
