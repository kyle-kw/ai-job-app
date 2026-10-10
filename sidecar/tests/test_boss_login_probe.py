from __future__ import annotations

import pathlib
import sys
import unittest
from unittest.mock import Mock, patch

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

import worker
from vendor import boss_cdp_raw as boss_vendor


class BossLoginProbeTests(unittest.TestCase):
    def test_probe_navigates_search_page_and_uses_capture_http_status(self):
        cdp = Mock()
        capture = Mock(last_http_status=429)
        capture.wait_next_response.side_effect = lambda **kwargs: (
            kwargs["trigger"]() or {"message": "访问频繁"}
        )
        with (
            patch.object(boss_vendor, "NetworkJoblistCapture", return_value=capture),
            patch.object(boss_vendor, "incr_request") as budget,
        ):
            result = boss_vendor.probe_login_state(
                cdp, "search-session", query="AI Agent", city_code="101020100"
            )

        self.assertIs(result.status, boss_vendor.LoginProbeStatus.RESTRICTED)
        self.assertIn("429", result.message)
        budget.assert_called_once()
        capture.enable.assert_called_once()
        navigation = cdp.send.call_args
        self.assertEqual(navigation.args[0], "Page.navigate")
        self.assertIn("/web/geek/job", navigation.args[1]["url"])
        self.assertIn("query=AI+Agent", navigation.args[1]["url"])
        self.assertIn("city=101020100", navigation.args[1]["url"])
        cdp.eval_js.assert_not_called()

    def test_response_classification_keeps_login_risk_and_transient_failures_distinct(self):
        cases = [
            (
                {"code": 0, "zpData": {"jobList": [{"salaryDesc": "20-40K"}]}},
                200,
                boss_vendor.LoginProbeStatus.AVAILABLE,
                False,
            ),
            (
                {"code": 0, "zpData": {"jobList": [{"salaryDesc": ""}]}},
                200,
                boss_vendor.LoginProbeStatus.UNAUTHENTICATED,
                False,
            ),
            (
                {"code": 0, "zpData": {"jobList": []}},
                200,
                boss_vendor.LoginProbeStatus.EMPTY,
                False,
            ),
            (
                {"code": 37, "message": "您的环境存在异常"},
                200,
                boss_vendor.LoginProbeStatus.RESTRICTED,
                False,
            ),
            (
                {"code": "9999", "message": "访问频繁，请稍后再试"},
                200,
                boss_vendor.LoginProbeStatus.RESTRICTED,
                False,
            ),
            ({}, 401, boss_vendor.LoginProbeStatus.UNAUTHENTICATED, False),
            ({}, 403, boss_vendor.LoginProbeStatus.RESTRICTED, False),
            ({}, 429, boss_vendor.LoginProbeStatus.RESTRICTED, False),
            ({}, 503, boss_vendor.LoginProbeStatus.RESPONSE_ERROR, True),
            (None, 200, boss_vendor.LoginProbeStatus.RESPONSE_ERROR, True),
            ({"code": 0, "zpData": {}}, 200, boss_vendor.LoginProbeStatus.RESPONSE_ERROR, True),
            (
                {"code": 7, "message": "业务异常"},
                200,
                boss_vendor.LoginProbeStatus.RESPONSE_ERROR,
                False,
            ),
        ]
        for response, status, expected, retryable in cases:
            with self.subTest(response=response, http_status=status):
                result = boss_vendor.classify_login_probe_response(response, http_status=status)
                self.assertIs(result.status, expected)
                self.assertEqual(result.retryable, retryable)

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
