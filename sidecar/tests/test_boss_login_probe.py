from __future__ import annotations

import json
import pathlib
import sys
import unittest
from unittest.mock import patch


ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

import worker
from vendor import boss_cdp_raw as boss_vendor


class BossLoginProbeTests(unittest.TestCase):
    def test_login_probe_uses_absolute_url(self):
        url = boss_vendor.build_login_probe_url("AI Agent", "101020100")

        self.assertTrue(url.startswith("https://www.zhipin.com/wapi/"))
        self.assertIn("query=AI+Agent", url)

    def test_login_probe_retries_during_captcha_navigation(self):
        test_case = self

        class NavigatingCdp:
            calls = 0

            @classmethod
            def eval_js(cls, js, _sid):
                cls.calls += 1
                test_case.assertIn("https://www.zhipin.com/wapi/", js)
                raise RuntimeError(
                    "Runtime.evaluate failed: SyntaxError: Failed to execute 'open' "
                    "on 'XMLHttpRequest': Invalid URL"
                )

        self.assertFalse(boss_vendor.probe_login_state(NavigatingCdp(), "session"))
        self.assertEqual(NavigatingCdp.calls, 1)

    def test_login_probe_treats_non_http_page_as_pending(self):
        test_case = self

        class PendingCdp:
            @staticmethod
            def eval_js(js, _sid):
                test_case.assertIn("location.protocol", js)
                return json.dumps({"__loginProbePending": True, "url": "about:blank"})

        self.assertFalse(boss_vendor.probe_login_state(PendingCdp(), "session"))

    def test_protocol_output_failure_does_not_mask_original_error(self):
        class InvalidStream:
            @staticmethod
            def write(_value):
                raise OSError(22, "Invalid argument")

            @staticmethod
            def flush():
                raise OSError(22, "Invalid argument")

        with patch.object(worker, "PROTOCOL_STDOUT", InvalidStream()):
            self.assertFalse(worker.emit("result", ok=False, error="original"))

        with patch.object(worker.sys, "stderr", InvalidStream()):
            try:
                raise RuntimeError("original")
            except RuntimeError:
                worker.print_current_exception()


if __name__ == "__main__":
    unittest.main()
