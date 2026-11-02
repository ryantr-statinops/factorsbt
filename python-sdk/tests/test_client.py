import json
import socket
import sys
import unittest
from pathlib import Path
from unittest.mock import patch
from urllib.error import URLError

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import Client, HealthResponse, HttpStatusError, ProtocolError, RequestTimeout, TransportError


class FakeResponse:
    def __init__(self, payload, status=200):
        self.payload = payload
        self.status = status

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def getcode(self):
        return self.status

    def read(self):
        return self.payload


class ClientHealthTests(unittest.TestCase):
    def test_health_gets_fixed_path_once_and_parses_response(self):
        payload = json.dumps(
            {"status": "ok", "service": "factorsbt-backend", "contract_version": "1.0.0"}
        ).encode()
        client = Client("http://127.0.0.1:8080/", timeout=2.5)
        with patch("factorsbt.client.urlopen", return_value=FakeResponse(payload)) as open_url:
            response = client.health()

        self.assertEqual(response, HealthResponse("ok", "factorsbt-backend", "1.0.0"))
        request = open_url.call_args.args[0]
        self.assertEqual(request.full_url, "http://127.0.0.1:8080/healthz")
        self.assertEqual(request.get_method(), "GET")
        self.assertEqual(open_url.call_args.kwargs["timeout"], 2.5)
        open_url.assert_called_once()

    def test_health_maps_timeout_connection_http_and_protocol_errors(self):
        client = Client("http://127.0.0.1:8080")
        with patch("factorsbt.client.urlopen", side_effect=socket.timeout):
            with self.assertRaises(RequestTimeout):
                client.health()
        with patch("factorsbt.client.urlopen", side_effect=URLError(ConnectionRefusedError())):
            with self.assertRaises(TransportError):
                client.health()
        with patch("factorsbt.client.urlopen", return_value=FakeResponse(b"{}", status=503)):
            with self.assertRaises(HttpStatusError) as caught:
                client.health()
        self.assertEqual(caught.exception.status, 503)
        with patch("factorsbt.client.urlopen", return_value=FakeResponse(b"not-json")):
            with self.assertRaises(ProtocolError):
                client.health()

    def test_client_rejects_invalid_url_or_timeout(self):
        with self.assertRaises(ValueError):
            Client("localhost:8080")
        with self.assertRaises(ValueError):
            Client("http://localhost", timeout=0)


if __name__ == "__main__":
    unittest.main()
