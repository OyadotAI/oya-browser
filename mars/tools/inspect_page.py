"""Open a URL in an ephemeral Mars window over the NDJSON agent channel, wait, print the page's
title, status, degraded list and diagnostics, and save a screenshot next to this script.

    python3 mars/tools/inspect_page.py <url> <seconds-to-wait> [out.png]

Uses mars/dist/Mars.app (build it with bash mars/packaging/build-app.sh). Launches with
--agent-control so the screenshot is allowed; the window is ephemeral and closes at the end."""
import json, subprocess, sys, time, threading
url, wait = sys.argv[1], float(sys.argv[2])
out_png = sys.argv[3] if len(sys.argv) > 3 else __file__.rsplit('/', 1)[0] + '/shot.png'
import pathlib
M = str(pathlib.Path(__file__).resolve().parents[1] / 'mars/dist/Mars.app/Contents/MacOS/mars')
p = subprocess.Popen([M, 'desktop', '--agent', '--agent-control', '--ephemeral', '--url', url], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
replies = {}
def reader():
    for line in p.stdout:
        try:
            frame = json.loads(line)
        except Exception:
            continue
        replies[frame.get('id')] = frame
threading.Thread(target=reader, daemon=True).start()
time.sleep(wait)
def ask(i, op, params):
    p.stdin.write(json.dumps({"version": 2, "id": i, "op": op, "params": params}) + "\n"); p.stdin.flush()
    for _ in range(300):
        if i in replies: return replies[i]
        time.sleep(0.1)
    return None
tabs = ask('t', 'tab.list', {})
tab = (tabs or {}).get('result', {}).get('tabs', [{}])[0].get('tab_id', 'tab-1')
page = ask('p', 'page.inspect', {"tab_id": tab})
out = {}
if page and 'result' in page:
    r = page['result']
    out = {k: r.get(k) for k in ('title', 'status', 'degraded', 'diagnostics', 'url')}
    out['nodes'] = len(r.get('nodes', [])) if isinstance(r.get('nodes'), list) else r.get('nodes')
    out['keys'] = list(r.keys())[:20]
else:
    out = {'reply': page}
print(json.dumps(out, indent=1)[:1500])
if page and 'result' in page:
    r = page['result']
    caps = ask('c', 'runtime.capabilities', {})
    ops = (caps or {}).get('result', {})
    schema = None
    def find(o):
        global schema
        if isinstance(o, dict):
            if o.get('op') == 'page.screenshot' or o.get('name') == 'page.screenshot': schema = o
            for v in o.values(): find(v)
        elif isinstance(o, list):
            for v in o: find(v)
    find(ops)
    pass
    status = ask('cs', 'control.status', {})
    epoch = (status or {}).get('result', {}).get('control_epoch')
    print('control', json.dumps((status or {}).get('result'))[:300])
    params = {"tab_id": tab, "page_id": r.get('page_id'), "expected_revision": r.get('revision'), "layout_generation": r.get('layout_generation')}
    shot = ask('s', 'page.screenshot', params)
    if shot and 'result' in shot:
        res = shot['result']; print('screenshot keys', list(res.keys())[:10])
        import base64
        for k, v in res.items():
            if isinstance(v, str) and len(v) > 1000:
                open(out_png, 'wb').write(base64.b64decode(v)); print('saved shot from', k, len(v)); break
    else:
        print('screenshot reply', json.dumps(shot)[:400])
p.stdin.write(json.dumps({"version": 2, "id": "q", "op": "runtime.shutdown", "params": {}}) + "\n"); p.stdin.flush()
try: p.wait(10)
except Exception: p.kill()
