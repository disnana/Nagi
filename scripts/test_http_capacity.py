import sys
import tempfile
import unittest
import urllib.request
from unittest import mock
from argparse import Namespace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from http_capacity import (
    LOCAL_OPENER,
    local_subprocess_environment,
    vegeta_attack_command,
    vegeta_report_command,
)


class VegetaReportCommandTests(unittest.TestCase):
    def test_loopback_clients_do_not_inherit_environment_proxies(self):
        # Passing ProxyHandler({}) removes urllib's environment-derived default.
        # On CPython the empty handler has no protocol methods and is therefore
        # omitted from the final opener, which is the desired closed state.
        self.assertFalse(any(
            isinstance(handler, urllib.request.ProxyHandler)
            for handler in LOCAL_OPENER.handlers
        ))

        with mock.patch.dict(
            "os.environ",
            {
                "HTTP_PROXY": "http://proxy.invalid:8080",
                "https_proxy": "http://proxy.invalid:8080",
            },
            clear=True,
        ):
            environment = local_subprocess_environment(2)
        self.assertEqual(environment["GOMAXPROCS"], "2")
        self.assertEqual(environment["NO_PROXY"], "*")
        self.assertEqual(environment["no_proxy"], "*")
        self.assertNotIn("HTTP_PROXY", environment)
        self.assertNotIn("https_proxy", environment)

    def test_pinned_json_report_uses_supported_flags(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder) / "report.jsonl"
            command = vegeta_report_command(["taskset", "-c", "2,3", "vegeta"], output)

        self.assertEqual(command[-3:], ["report", "-type=json", "-output=" + str(output)])
        self.assertFalse(any(argument.startswith("-buckets=") for argument in command))

    def test_periodic_report_is_explicit(self):
        command = vegeta_report_command(["vegeta"], Path("report.jsonl"), 60)

        self.assertEqual(command[-1], "-every=60s")

    def test_attack_does_not_follow_redirects(self):
        args = Namespace(
            max_workers=512,
            max_connections=256,
            local_address="127.0.20.1",
        )
        command = vegeta_attack_command(
            ["vegeta"], Path("targets.txt"), 100, 3, args
        )

        self.assertIn("-redirects=0", command)


if __name__ == "__main__":
    unittest.main()
