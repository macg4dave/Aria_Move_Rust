"""Exercise the installed service's authenticated RPC and completion hook offline."""
import functools
import http.server
import json
import pathlib
import tempfile
import threading
import time
import urllib.request

secret = pathlib.Path('/var/lib/aria-move/rpc-secret').read_text().strip()


def rpc(method, params):
    body = json.dumps(dict(jsonrpc='2.0', id='test', method=method,
                           params=['token:' + secret, *params])).encode()
    request = urllib.request.Request('http://127.0.0.1:6800/jsonrpc', data=body)
    with urllib.request.urlopen(request, timeout=5) as response:
        result = json.load(response)
    assert 'error' not in result, result
    return result['result']


with tempfile.TemporaryDirectory() as directory:
    payload = b'Installed aria2 completion hook test\n' * 1024
    pathlib.Path(directory, 'package-test.bin').write_bytes(payload)
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=directory)
    with http.server.ThreadingHTTPServer(('127.0.0.1', 0), handler) as server:
        threading.Thread(target=server.serve_forever, daemon=True).start()
        rpc('aria2.addUri', [[f'http://127.0.0.1:{server.server_port}/package-test.bin']])
        destination = pathlib.Path('/var/lib/aria-move/completed/package-test.bin')
        for _ in range(200):
            if destination.exists():
                break
            time.sleep(0.1)
        assert destination.read_bytes() == payload
        assert not pathlib.Path('/var/lib/aria-move/incoming/package-test.bin').exists()
        destination.unlink()
        server.shutdown()
print('Authenticated RPC download and completion hook passed')
