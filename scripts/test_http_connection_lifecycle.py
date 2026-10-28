import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from http_connection_lifecycle import PARTIAL_BODY, read_http_response


class FakeConnection:
    def __init__(self, chunks):
        self.chunks = list(chunks)

    def recv(self, _size):
        return self.chunks.pop(0) if self.chunks else b""

    def settimeout(self, _seconds):
        pass


class HttpResponseReaderTests(unittest.TestCase):
    def test_partial_body_fixture_uses_the_permitted_authority(self):
        self.assertIn(b"Host: localhost:8080\r\n", PARTIAL_BODY)
        header, body = PARTIAL_BODY.split(b"\r\n\r\n", 1)
        declared = int(next(
            line.split(b":", 1)[1].strip()
            for line in header.split(b"\r\n")
            if line.lower().startswith(b"content-length:")
        ))
        self.assertLess(len(body), declared)

    def test_reads_split_header_and_body(self):
        connection = FakeConnection([
            b"HTTP/1.1 200 OK\r\nContent-Len",
            b"gth: 2\r\n\r\n",
            b"ok",
        ])

        self.assertEqual(read_http_response(connection), (b"HTTP/1.1 200 OK", b"ok"))

    def test_rejects_eof_before_header(self):
        with self.assertRaisesRegex(RuntimeError, "header completed"):
            read_http_response(FakeConnection([b"HTTP/1.1 200 OK\r\n", b""]))

    def test_rejects_eof_before_body(self):
        with self.assertRaisesRegex(RuntimeError, "body completed"):
            read_http_response(FakeConnection([
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n",
                b"",
            ]))

    def test_rejects_excessive_body_before_reading_it(self):
        with self.assertRaisesRegex(RuntimeError, "body exceeds"):
            read_http_response(FakeConnection([
                b"HTTP/1.1 200 OK\r\nContent-Length: 1048577\r\n\r\n",
            ]))

    def test_rejects_nonpositive_total_deadline(self):
        with self.assertRaisesRegex(RuntimeError, "total deadline"):
            read_http_response(FakeConnection([b"HTTP/1.1 200 OK\r\n"]), deadline_seconds=0)


if __name__ == "__main__":
    unittest.main()
