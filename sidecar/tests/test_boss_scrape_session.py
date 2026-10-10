from __future__ import annotations

import pathlib
import sys
import unittest
from unittest.mock import patch

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

import browser_ops
from vendor import boss_cdp_raw as boss


class FakeBoss:
    DEFAULT_CDP_DATA_DIR = "test-profile"

    def __init__(self, responses, *, login_succeeds=True):
        self.LoginGateError = boss.LoginGateError
        self.LoginProbeStatus = boss.LoginProbeStatus
        self.responses = iter(responses)
        self.login_succeeds = login_succeeds
        self.setup_waits = []
        self.searches = []
        self.stopped = False

    @staticmethod
    def resolve_city(_city):
        return "上海", "101020100"

    def run_setup_chrome(self, _port, **kwargs):
        self.setup_waits.append(kwargs["wait_login"])
        if kwargs["wait_login"] and not self.login_succeeds:
            print("等待登录超时")
            return 1
        return 0

    def scrape_list(self, keyword, city, pages, filters, _output, **kwargs):
        self.searches.append((keyword, city, pages, filters))
        response = next(self.responses)
        if callable(response):
            return response(kwargs["on_job"])
        if isinstance(response, Exception):
            raise response
        for job in response["jobs"]:
            kwargs["on_job"](job)
        return response

    def stop_cdp_chrome(self, _profile):
        self.stopped = True
        return 0

    @staticmethod
    def chrome_pids_for_user_data_dir(_profile):
        return []


class BossScrapeSessionTests(unittest.TestCase):
    params = {
        "keyword": "Python",
        "city": "上海",
        "pages": 2,
        "salary": "406",
        "completedDetailExternalIds": ["job-1"],
    }
    raw_job = {"job_id": "job-1", "title": "Python 工程师", "salary": "20-30K"}

    def success(self):
        return {"jobs": [self.raw_job], "total": 1}

    @staticmethod
    def gate(status):
        return boss.LoginGateError("搜索不可用", result=boss.LoginProbeResult(status))

    def run_scrape(self, fake):
        events = []
        with (
            patch.object(browser_ops, "load_boss_module", return_value=fake),
            patch.object(
                browser_ops,
                "emit",
                side_effect=lambda kind, **payload: events.append((kind, payload)),
            ),
        ):
            result = browser_ops.scrape_jobs(self.params)
        return result, events

    def test_valid_session_uses_actual_search_without_login_probe(self):
        fake = FakeBoss([self.success()])
        result, events = self.run_scrape(fake)

        self.assertEqual(fake.setup_waits, [False])
        self.assertEqual(fake.searches, [("Python", "101020100", 2, {"salary": "406"})])
        self.assertEqual(len(result["jobs"]), 1)
        self.assertEqual([kind for kind, _ in events].count("job"), 1)
        self.assertTrue(fake.stopped)

    def test_expired_session_waits_for_login_then_retries_same_search_once(self):
        fake = FakeBoss([self.gate(boss.LoginProbeStatus.UNAUTHENTICATED), self.success()])
        result, events = self.run_scrape(fake)

        self.assertEqual(fake.setup_waits, [False, True])
        self.assertEqual(len(fake.searches), 2)
        self.assertEqual(fake.searches[0], fake.searches[1])
        self.assertEqual(len(result["jobs"]), 1)
        self.assertTrue(any("登录已过期" in payload.get("message", "") for _, payload in events))
        self.assertTrue(fake.stopped)

    def test_restricted_or_unknown_response_stops_without_repeated_login(self):
        for status in (boss.LoginProbeStatus.RESTRICTED, boss.LoginProbeStatus.RESPONSE_ERROR):
            with self.subTest(status=status):
                fake = FakeBoss([self.gate(status)])
                with self.assertRaises(boss.LoginGateError):
                    self.run_scrape(fake)
                self.assertEqual(fake.setup_waits, [False])
                self.assertEqual(len(fake.searches), 1)
                self.assertTrue(fake.stopped)

    def test_login_expiry_after_streamed_jobs_preserves_jobs_without_restarting(self):
        def expired_page(on_job):
            on_job(self.raw_job)
            raise self.gate(boss.LoginProbeStatus.UNAUTHENTICATED)

        fake = FakeBoss([expired_page])
        with self.assertRaises(boss.LoginGateError):
            self.run_scrape(fake)
        self.assertEqual(fake.setup_waits, [False])
        self.assertEqual(len(fake.searches), 1)
        self.assertTrue(fake.stopped)

    def test_second_login_failure_is_not_retried_again(self):
        fake = FakeBoss([self.gate(boss.LoginProbeStatus.UNAUTHENTICATED)] * 2)
        with self.assertRaises(boss.LoginGateError):
            self.run_scrape(fake)
        self.assertEqual(fake.setup_waits, [False, True])
        self.assertEqual(len(fake.searches), 2)
        self.assertTrue(fake.stopped)

    def test_manual_login_timeout_still_cleans_up_browser(self):
        fake = FakeBoss([self.gate(boss.LoginProbeStatus.UNAUTHENTICATED)], login_succeeds=False)
        with self.assertRaisesRegex(RuntimeError, "等待登录超时"):
            self.run_scrape(fake)
        self.assertEqual(fake.setup_waits, [False, True])
        self.assertEqual(len(fake.searches), 1)
        self.assertTrue(fake.stopped)


if __name__ == "__main__":
    unittest.main()
