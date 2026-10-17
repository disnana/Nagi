"""Owned loopback wrong-CA oracle; keys generated only inside fresh work.

Dedicated capsule leader. EOF on inherited stop fd closes the actual server.
"""
import argparse
import base64
import http.server
import importlib.util
import json
import os
from pathlib import Path
import select
import socket
import stat
import sys


def load_fixture(path):
    spec = importlib.util.spec_from_file_location('bound_fixture', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def log_message(self, *_args):
        pass
    def do_GET(self):
        # Only root, no body, no redirects, credentials or reflected input.
        if self.path != '/' or self.headers.get('Content-Length', '0') != '0' or self.headers.get('Transfer-Encoding'):
            self.close_connection = True
            return
        self.send_response(200)
        self.send_header('Content-Length', '2')
        self.send_header('Connection', 'close')
        self.end_headers()
        self.wfile.write(b'ok')
        self.close_connection = True


class Server(http.server.HTTPServer):
    request_queue_size = 8
    def get_request(self):
        sock, addr = self.socket.accept()
        try:
            sock.settimeout(5)
            return self.context.wrap_socket(sock, server_side=True), addr
        except BaseException:
            sock.close()
            raise
    def handle_error(self, *_args):
        pass  # Never print request/headers or exception data.


def run(work, fixture, stop_fd):
    os.umask(0o077)
    info = os.fstat(stop_fd)
    if not stat.S_ISFIFO(info.st_mode):
        raise ValueError('stop pipe required')
    owned = work / 'negative'
    owned.mkdir(mode=0o700)  # Existing path refuses; never resets or reuses keys.
    module = load_fixture(fixture)
    context = module.certificates(owned, 'wrong-ca')
    with Server(('127.0.0.1', 0), Handler) as server:
        server.context, server.timeout = context, .1
        port = server.server_address[1]
        if not 1024 <= port <= 65535:
            raise ValueError('owned high port required')
        # Public certificate only, bounded generated source. No key/DB reads.
        ca = (owned / 'ca.pem').read_bytes()
        if not 0 < len(ca) <= 16384:
            raise ValueError('certificate bound')
        module.atomic_json(work / 'negative.json', {'schema_version': 1,
            'ca_origin': f'https://localhost:{port}', 'ca_pem': base64.b64encode(ca).decode('ascii')})
        while True:
            if select.select([stop_fd], [], [], 0)[0]:
                if os.read(stop_fd, 1) != b'':
                    raise ValueError('stop EOF required')
                break
            server.handle_request()
    os.close(stop_fd)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--fixture', required=True)
    parser.add_argument('--stop-fd', type=int, required=True)
    args = parser.parse_args()
    try:
        run(args.work, args.fixture, args.stop_fd)
    except BaseException:
        raise SystemExit(1) from None
