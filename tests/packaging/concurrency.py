"""Real aria2 hooks: a 2 GiB cross-device copy plus ten concurrent 1 MiB jobs.

Run only in a disposable systemd container, with completed on a separate
filesystem of at least 3 GiB. Pausing the partially copied large hook makes the
overlap deterministic even on fast storage. No production test switches needed.
"""
import hashlib
import concurrent.futures
import http.server
import json
import os
import pathlib
import signal
import threading
import time
import urllib.request
import uuid

assert os.geteuid() == 0
assert pathlib.Path('/run/systemd/container').read_text().strip() in ('docker', 'systemd-nspawn')
incoming = pathlib.Path('/var/lib/aria-move/incoming')
completed = pathlib.Path('/var/lib/aria-move/completed')
assert incoming.stat().st_dev != completed.stat().st_dev, 'Must force actual copying across filesystems'
assert not list(incoming.iterdir()) and not list(completed.iterdir()), 'Requires fresh test storage'
assert os.statvfs(completed).f_bavail * os.statvfs(completed).f_frsize > 3 * 1024**3 - 1024**2
secret = pathlib.Path('/var/lib/aria-move/rpc-secret').read_text().strip()
prefix = 'concurrency-' + uuid.uuid4().hex[:12]
large = prefix + '-large.bin'
small = [prefix + f'-small-{index:02}.bin' for index in range(10)]
block = bytes(range(256)) * 4096
payloads = {large: (block, 2048)}  # 2 GiB, genuinely written and read back.
payloads.update({name: (bytes([index + 1]) * 1024**2, 1) for index, name in enumerate(small)})
expected = {}
for name, (data, repeats) in payloads.items():
    digest = hashlib.sha256()
    for _ in range(repeats):
        digest.update(data)
    expected[name] = digest.hexdigest()


def rpc(method, params):
    request = urllib.request.Request('http://127.0.0.1:6800/jsonrpc', data=json.dumps({
        'jsonrpc': '2.0', 'id': 'concurrency', 'method': method,
        'params': ['token:' + secret, *params]}).encode())
    with urllib.request.urlopen(request, timeout=10) as response:
        result = json.load(response)
    assert 'error' not in result, result
    return result['result']


def wait_for(predicate, message, timeout=120):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.005)
    raise AssertionError(message)


def hooks():
    found = {}
    for entry in pathlib.Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            args = (entry / 'cmdline').read_bytes().split(b'\0')
        except (FileNotFoundError, ProcessLookupError):
            continue
        if b'/usr/bin/aria_move' in args:
            for name in payloads:
                if os.fsencode(incoming / name) in args:
                    found[name] = int(entry.name)
    return found


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        name = self.path.removeprefix('/')
        if name not in payloads:
            self.send_error(404)
            return
        data, repeats = payloads[name]
        self.send_response(200)
        self.send_header('Content-Length', str(len(data) * repeats))
        self.send_header('Accept-Ranges', 'none')
        self.end_headers()
        for _ in range(repeats):
            self.wfile.write(data)

    def log_message(self, *_args):
        pass


paused = None
old_parallelism = rpc('aria2.getGlobalOption', [])['max-concurrent-downloads']
started = time.monotonic()
with http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler) as server:
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        rpc('aria2.changeGlobalOption', [{'max-concurrent-downloads': '20'}])

        def queue(name):
            return rpc('aria2.addUri', [[f'http://127.0.0.1:{server.server_port}/{name}'],
                                       {'split': '1', 'max-connection-per-server': '1'}])

        print('Downloading 2 GiB through aria2; waiting for a partial cross-device copy', flush=True)
        gids = {large: queue(large)}

        def partial_copy():
            running = hooks()
            if large not in running:
                return None
            for stage in completed.glob('.aria_move.stage.*/' + large):
                try:
                    size = stage.stat().st_size
                except FileNotFoundError:
                    continue
                if 0 < size < 2 * 1024**3:
                    return running[large]
            return None

        paused = wait_for(partial_copy, 'Did not observe a partial 2 GiB cross-device copy')
        os.kill(paused, signal.SIGSTOP)
        print('Large hook paused during copying; submitting ten 1 MiB downloads', flush=True)
        with concurrent.futures.ThreadPoolExecutor(max_workers=10) as pool:
            gids.update(zip(small, pool.map(queue, small)))
        wait_for(lambda: all((completed / name).exists() for name in small),
                 'Small moves blocked behind the paused large copy', timeout=30)
        assert not (completed / large).exists()
        assert (incoming / large).stat().st_size == 2 * 1024**3
        assert all(not (incoming / name).exists() for name in small)
        print('All ten small downloads moved while the large hook remains paused mid-copy', flush=True)
        resumed = time.monotonic()
        os.kill(paused, signal.SIGCONT)
        paused = None
        wait_for(lambda: all((completed / name).exists() for name in payloads)
                 and not hooks(), 'Concurrent moves did not finish', timeout=300)
        finished = time.monotonic()
        for name, (data, repeats) in payloads.items():
            destination = completed / name
            assert destination.stat().st_size == len(data) * repeats, name
            with destination.open('rb') as stream:
                assert hashlib.file_digest(stream, 'sha256').hexdigest() == expected[name], name
            assert not (incoming / name).exists(), name
            assert rpc('aria2.tellStatus', [gids[name], ['status']])['status'] == 'complete'
        assert not list(completed.glob('.aria_move.stage.*')), 'Staging files leaked'
        assert len(list(completed.iterdir())) == 11, 'Missing, duplicate or unexpected outputs'
        print(f'PASS: 2 GiB + 10 x 1 MiB, 11 real aria2 hooks, all SHA-256 hashes matched; '
              f'moves drained {finished - resumed:.2f}s after resume; '
              f'total {time.monotonic() - started:.2f}s', flush=True)
    finally:
        if paused is not None:
            os.kill(paused, signal.SIGCONT)
        rpc('aria2.changeGlobalOption', [{'max-concurrent-downloads': old_parallelism}])
        server.shutdown()
