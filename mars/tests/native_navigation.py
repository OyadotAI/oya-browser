"""Exercise native Mars address/Stop queues with isolated profiles and loopback delays."""
import argparse
import http.server
import json
import pathlib
import queue
import subprocess
import threading
import time

WAIT_SECONDS = 8
POLL_SECONDS = 0.02
STOP_SECONDS = 1
ACK_SECONDS = 0.25


class Fixture(http.server.BaseHTTPRequestHandler):
    """A stalled response never blocks the fixture's independent replacement request."""
    requests = queue.Queue()
    release = threading.Event()
    agent_release = threading.Event()

    def do_GET(self):
        """Report admission before waiting, so tests cancel actual outstanding network I/O."""
        self.requests.put(self.path)
        if self.path.startswith('/slow'):
            self.release.wait(WAIT_SECONDS)
        if self.path == '/agent-wait':
            self.agent_release.wait(WAIT_SECONDS)
        content = '<script>throw Error("fixture load failed")</script>' if self.path == '/broken' else ''
        if self.path == '/links':
            content = '<style>@-moz-keyframes spinner{0%,39%,100%{opacity:0}40%{opacity:1}}</style><a id=next href=/destination>Open destination</a>'
        body = f'<html><title>{self.path}</title><body><h1>{self.path}</h1>{content}</body></html>'.encode()
        try:
            self.send_response(200)
            self.send_header('Content-Type', 'text/html')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, *_):
        """Synthetic request paths are recorded explicitly, without noisy HTTP logs."""


class Mars:
    """Own one ephemeral native process and match responses by their protocol identity."""
    def __init__(self, binary, origin, output):
        self.responses = queue.Queue()
        self.sequence = 0
        self.process = subprocess.Popen(
            [str(binary), 'desktop', '--agent', '--ephemeral', '--fixture-origin', origin,
             '--capture-dir', str(output), '--url', origin + '/base'], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=(output / 'stderr.log').open('w'), text=True)
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        """Only protocol JSON reaches the response queue."""
        for line in self.process.stdout:
            self.responses.put(json.loads(line))

    def send(self, operation, params):
        """Submission and acknowledgement remain separate to test consecutive UI input."""
        self.sequence += 1
        identity = str(self.sequence)
        self.process.stdin.write(json.dumps({'version': 2, 'id': identity, 'op': operation, 'params': params}) + '\n')
        self.process.stdin.flush()
        return identity

    def answer(self, identity):
        """Fail boundedly instead of leaving a stranded native app after a regression."""
        until = time.monotonic() + WAIT_SECONDS
        while time.monotonic() < until:
            reply = self.responses.get(timeout=WAIT_SECONDS)
            if reply.get('id') == identity:
                assert 'error' not in reply, reply.get('error')
                return reply['result']
        raise TimeoutError(identity)

    def call(self, operation, params):
        """Ordinary observations wait for their own native main-thread acknowledgement."""
        return self.answer(self.send(operation, params))

    def human(self, action, text=''):
        """Test-only input enters the real Canvas action queue used by the address control."""
        return self.call('window.human', {'action': action, 'text': text})

    def inspect(self):
        """Observe native shell state even while its runtime worker is blocked."""
        return self.call('window.inspect', {})

    def wait(self, predicate):
        """Poll the UI through its native API, with an independent integration deadline."""
        until = time.monotonic() + WAIT_SECONDS
        while time.monotonic() < until:
            state = self.inspect()
            if predicate(state):
                return state
            time.sleep(POLL_SECONDS)
        raise AssertionError(state)

    def close(self):
        """Terminate only the owned fixture app; never touch the user's running browser."""
        self.process.stdin.close()
        try:
            self.process.wait(timeout=WAIT_SECONDS)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=WAIT_SECONDS)


def accepted(path):
    """Wait until Relay has sent the named request to the delayed endpoint."""
    until = time.monotonic() + WAIT_SECONDS
    while time.monotonic() < until:
        if Fixture.requests.get(timeout=WAIT_SECONDS) == path:
            return
    raise AssertionError(path)


def run(binary, output):
    """Supersession and repeated Stop must finish before any delayed response is released."""
    output.mkdir(parents=True, exist_ok=True)
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    origin = f'http://127.0.0.1:{server.server_port}'
    mars = Mars(binary, origin, output)
    results = []
    try:
        mars.wait(lambda state: (state.get('page') or {}).get('url') == origin + '/base' and not state['navigation']['busy'])
        mars.human('enter', origin + '/slow-a')
        accepted('/slow-a')
        loading = mars.wait(lambda state: state['navigation']['stop_visible'])
        assert loading['navigation']['committed'] == origin + '/base', loading
        started = time.monotonic()
        second = mars.send('window.human', {'action': 'enter', 'text': origin + '/slow-b'})
        third = mars.send('window.human', {'action': 'enter', 'text': origin + '/c'})
        mars.answer(second)
        mars.answer(third)
        current = mars.wait(lambda state: (state.get('page') or {}).get('url') == origin + '/c' and not state['navigation']['busy'])
        elapsed = time.monotonic() - started
        assert elapsed < STOP_SECONDS, elapsed
        assert current['error'] is None, current
        results.append({'check': 'A-B-C replacement before delayed response', 'seconds': elapsed})
        for attempt in range(5):
            path = f'/slow-stop-{attempt}'
            mars.human('enter', origin + path)
            accepted(path)
            started = time.monotonic()
            mars.human('stop')
            observed = mars.wait(lambda state: state['navigation']['stopping'] or not state['navigation']['busy'])
            ack = time.monotonic() - started
            assert ack < ACK_SECONDS, ack
            settled = mars.wait(lambda state: not state['navigation']['busy'])
            elapsed = time.monotonic() - started
            assert elapsed < STOP_SECONDS, elapsed
            assert settled['page']['url'] == origin + '/c', settled
            assert settled['error'] is None, settled
            results.append({'check': f'Stop {attempt + 1}', 'ack_seconds': ack, 'settled_seconds': elapsed})
        Fixture.release.set()
        time.sleep(POLL_SECONDS)
        assert mars.inspect()['page']['url'] == origin + '/c'
        mars.human('enter', origin + '/broken')
        mars.wait(lambda state: not state['navigation']['busy'] and 'Some page features' in (state['error'] or ''))
        mars.human('details')
        expanded = mars.wait(lambda state: 'fixture load failed' in (state['error'] or ''))
        results.append({'check': 'failure details retain the concrete script error', 'message': expanded['error']})
        mars.human('dismiss')
        mars.wait(lambda state: state['error'] is None)
        time.sleep(POLL_SECONDS)
        assert mars.inspect()['error'] is None
        mars.human('enter', origin + '/links')
        links = mars.wait(lambda state: not state['navigation']['busy'] and state['page']['url'] == origin + '/links')
        link = next(node for node in links['page']['nodes'] if node['html_id'] == 'next')
        assert link['enabled'] and link['visible'], link
        bounds = link['bounds']
        mars.call('window.human', {'action': 'click', 'point': [bounds['x'] + bounds['width'] / 2, bounds['y'] + bounds['height'] / 2]})
        mars.wait(lambda state: not state['navigation']['busy'] and state['page']['url'] == origin + '/destination')
        mars.human('back')
        mars.wait(lambda state: not state['navigation']['busy'] and state['page']['url'] == origin + '/links')
        results.append({'check': 'native click and Back work with vendor animation CSS'})
        background = mars.call('tab.open', {})
        agent = mars.send('page.navigate', {'tab_id': background['tab_id'], 'control_epoch': background['control_epoch'], 'url': origin + '/agent-wait'})
        accepted('/agent-wait')
        mars.human('reload')
        mars.wait(lambda state: state['navigation']['busy'])
        Fixture.agent_release.set()
        mars.answer(agent)
        mars.wait(lambda state: not state['navigation']['busy'])
        mars.human('enter', origin + '/after-agent')
        mars.wait(lambda state: not state['navigation']['busy'] and state['page']['url'] == origin + '/after-agent')
        results.append({'check': 'overlapping agent completion releases the next human navigation'})
        (output / 'report.json').write_text(json.dumps(results, indent=2))
        print(json.dumps(results, indent=2))
    finally:
        Fixture.release.set()
        Fixture.agent_release.set()
        mars.close()
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=pathlib.Path)
    parser.add_argument('--output', type=pathlib.Path, default=pathlib.Path('/private/tmp/mars-native-navigation'))
    arguments = parser.parse_args()
    run(arguments.binary.resolve(), arguments.output)
