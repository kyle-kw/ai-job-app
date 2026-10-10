from __future__ import annotations

import pathlib
import sys
import unittest
from contextlib import redirect_stdout
from io import StringIO
from unittest.mock import Mock, patch

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from vendor import boss_cdp_raw as boss

LOGIN_URL = "https://www.zhipin.com/web/user/"
SEARCH_URL = "https://www.zhipin.com/web/geek/job?query=Java&city=101020100"


class FakeClock:
    def __init__(self):
        self.current = 0.0
        self.sleeps = []

    def time(self):
        return self.current

    def sleep(self, seconds):
        self.sleeps.append(seconds)
        self.current += seconds


class FakeLoginCDP:
    def __init__(self, name, clock, urls=(), failures=None):
        self.name = name
        self.clock = clock
        self.urls = list(urls)
        self.failures = dict(failures or {})
        self.events = []
        self.sent = []
        self.url_reads = []
        self.closed = False

    @property
    def target(self):
        return f"{self.name}-target"

    @property
    def session(self):
        return f"{self.name}-session"

    def send(self, method, params=None, sid=None):
        params = params or {}
        self.sent.append((method, params, sid))
        if method in self.failures:
            raise self.failures[method]
        if method == "Target.createTarget":
            return {"result": {"targetId": self.target}}
        if method == "Target.attachToTarget":
            return {"result": {"sessionId": self.session}}
        return {"result": {}}

    def eval_js(self, expression, sid):
        self.url_reads.append((expression, sid, self.clock.current))
        value = self.urls.pop(0) if len(self.urls) > 1 else self.urls[0]
        if isinstance(value, Exception):
            raise value
        return value

    def close(self):
        self.closed = True

    def closed_targets(self):
        return [
            params["targetId"]
            for method, params, _sid in self.sent
            if method == "Target.closeTarget"
        ]

    def navigations(self):
        return [
            (params["url"], sid) for method, params, sid in self.sent if method == "Page.navigate"
        ]


class BossLoginWaitTests(unittest.TestCase):
    def setUp(self):
        self.clock = FakeClock()
        self.previous_budget = boss._request_counter
        boss._request_counter = 0

    def tearDown(self):
        boss._request_counter = self.previous_budget

    def run_wait(self, login, probe, outcomes, timeout=20):
        with (
            patch.object(boss, "CDPSession", side_effect=[login, probe]) as connect,
            patch.object(boss.time, "time", side_effect=self.clock.time),
            patch.object(boss.time, "sleep", side_effect=self.clock.sleep),
            patch.object(boss, "probe_login_state", side_effect=outcomes) as check,
            redirect_stdout(StringIO()),
        ):
            result = boss.wait_for_login(timeout=timeout, interval=3)
        return result, connect, check

    def test_pending_login_never_creates_probe_or_changes_foreground_page(self):
        pending_urls = [
            LOGIN_URL,
            "https://www.zhipin.com/web/user/?returnUrl=%2Fweb%2Fgeek%2Fjob",
            "https://www.zhipin.com/login",
            "https://www.zhipin.com/web/user/login",
            "https://www.zhipin.com/web/geek/job/_security_check",
            "about:blank",
            "https://example.com/web/geek/job",
            "https://www.zhipin.com.example.com/web/geek/job",
        ]
        for url in pending_urls:
            with self.subTest(url=url):
                self.clock = FakeClock()
                login = FakeLoginCDP("login", self.clock, [url])
                probe = FakeLoginCDP("probe", self.clock)
                result, connect, check = self.run_wait(login, probe, [], timeout=5)

                self.assertFalse(result)
                self.assertEqual(connect.call_count, 1)
                check.assert_not_called()
                self.assertEqual(login.navigations(), [(LOGIN_URL, login.session)])
                self.assertEqual(login.closed_targets(), [])
                self.assertTrue(login.closed)
                self.assertEqual(probe.sent, [])
                self.assertFalse(probe.closed)
                self.assertEqual(self.clock.current, 5)
                self.assertTrue(login.url_reads)
                self.assertTrue(
                    all(
                        expression == "location.href" and sid == login.session
                        for expression, sid, _time in login.url_reads
                    )
                )

    def test_real_probe_search_navigation_is_isolated_after_manual_login(self):
        login = FakeLoginCDP("login", self.clock, [LOGIN_URL, LOGIN_URL, SEARCH_URL])
        probe = FakeLoginCDP("probe", self.clock)
        capture = Mock(last_http_status=200)
        capture.wait_next_response.side_effect = lambda **kwargs: (
            kwargs["trigger"]() or {"code": 0, "zpData": {"jobList": [{"salaryDesc": "20-40K"}]}}
        )
        connection_times = []

        def connect(_port):
            connection_times.append(self.clock.current)
            return login if len(connection_times) == 1 else probe

        with (
            patch.object(boss, "CDPSession", side_effect=connect),
            patch.object(boss.time, "time", side_effect=self.clock.time),
            patch.object(boss.time, "sleep", side_effect=self.clock.sleep),
            patch.object(boss, "NetworkJoblistCapture", return_value=capture) as capture_factory,
            redirect_stdout(StringIO()),
        ):
            result = boss.wait_for_login(timeout=20, interval=3)

        self.assertTrue(result)
        self.assertEqual(login.navigations(), [(LOGIN_URL, login.session)])
        self.assertEqual(connection_times, [0, 6])
        capture_factory.assert_called_once_with(probe, probe.session)
        capture.enable.assert_called_once()
        self.assertEqual(len(probe.navigations()), 1)
        search_url, search_session = probe.navigations()[0]
        self.assertIn("/web/geek/job", search_url)
        self.assertEqual(search_session, probe.session)
        self.assertEqual(login.closed_targets(), [login.target])
        self.assertEqual(probe.closed_targets(), [probe.target])
        self.assertTrue(login.closed)
        self.assertTrue(probe.closed)
        self.assertEqual(
            [
                params["background"]
                for method, params, _sid in login.sent
                if method == "Target.createTarget"
            ],
            [False],
        )
        self.assertEqual(
            [
                params["background"]
                for method, params, _sid in probe.sent
                if method == "Target.createTarget"
            ],
            [True],
        )

    def test_leaving_login_url_does_not_count_as_authenticated(self):
        login = FakeLoginCDP("login", self.clock, [SEARCH_URL])
        probe = FakeLoginCDP("probe", self.clock)
        unauthenticated = boss.LoginProbeResult(boss.LoginProbeStatus.UNAUTHENTICATED)
        result, connect, check = self.run_wait(
            login, probe, [unauthenticated, unauthenticated], timeout=5
        )

        self.assertFalse(result)
        self.assertEqual(connect.call_count, 2)
        self.assertEqual(check.call_count, 2)
        self.assertTrue(all(call.args == (probe, probe.session) for call in check.call_args_list))
        self.assertEqual(login.navigations(), [(LOGIN_URL, login.session)])
        self.assertEqual(login.closed_targets(), [])
        self.assertEqual(probe.closed_targets(), [probe.target])
        self.assertTrue(login.closed)
        self.assertTrue(probe.closed)

    def test_navigation_read_errors_wait_for_user_instead_of_using_search(self):
        login = FakeLoginCDP(
            "login",
            self.clock,
            [
                RuntimeError("Execution context was destroyed"),
                TimeoutError("navigation"),
                SEARCH_URL,
            ],
        )
        probe = FakeLoginCDP("probe", self.clock)
        available = boss.LoginProbeResult(boss.LoginProbeStatus.AVAILABLE)
        result, connect, check = self.run_wait(login, probe, [available])

        self.assertTrue(result)
        self.assertEqual(self.clock.sleeps, [3, 3])
        self.assertEqual(connect.call_count, 2)
        check.assert_called_once()
        self.assertEqual(check.call_args.args, (probe, probe.session))
        self.assertEqual(login.navigations(), [(LOGIN_URL, login.session)])
        self.assertTrue(login.closed)
        self.assertTrue(probe.closed)

    def test_url_read_errors_are_bounded_by_login_timeout(self):
        login = FakeLoginCDP("login", self.clock, [RuntimeError("frame navigating")])
        probe = FakeLoginCDP("probe", self.clock)
        result, connect, check = self.run_wait(login, probe, [], timeout=5)

        self.assertFalse(result)
        self.assertEqual(self.clock.current, 5)
        self.assertEqual(connect.call_count, 1)
        check.assert_not_called()
        self.assertEqual(login.closed_targets(), [])
        self.assertTrue(login.closed)

    def test_restricted_and_terminal_probe_errors_clean_background_and_preserve_login(self):
        outcomes = [
            boss.LoginProbeResult(boss.LoginProbeStatus.RESTRICTED, code=37),
            boss.LoginProbeResult(boss.LoginProbeStatus.RESPONSE_ERROR, code=7, message="业务异常"),
            RuntimeError("probe connection failed"),
        ]
        for outcome in outcomes:
            with self.subTest(outcome=outcome):
                self.clock = FakeClock()
                login = FakeLoginCDP("login", self.clock, [SEARCH_URL])
                probe = FakeLoginCDP("probe", self.clock)
                result, connect, check = self.run_wait(login, probe, [outcome])

                self.assertFalse(result)
                self.assertEqual(connect.call_count, 2)
                check.assert_called_once()
                self.assertEqual(check.call_args.args, (probe, probe.session))
                self.assertEqual(self.clock.sleeps, [])
                self.assertEqual(login.closed_targets(), [])
                self.assertEqual(probe.closed_targets(), [probe.target])
                self.assertTrue(login.closed)
                self.assertTrue(probe.closed)

    def test_probe_retries_reuse_only_background_session(self):
        login = FakeLoginCDP("login", self.clock, [SEARCH_URL])
        probe = FakeLoginCDP("probe", self.clock)
        outcomes = [
            TimeoutError("CDP recv timed out"),
            boss.LoginProbeResult(boss.LoginProbeStatus.EMPTY),
            boss.LoginProbeResult(boss.LoginProbeStatus.AVAILABLE),
        ]
        result, connect, check = self.run_wait(login, probe, outcomes, timeout=100)

        self.assertTrue(result)
        self.assertEqual(connect.call_count, 2)
        self.assertEqual(check.call_count, 3)
        self.assertTrue(all(call.args == (probe, probe.session) for call in check.call_args_list))
        self.assertEqual(self.clock.sleeps, [3, 6])
        self.assertEqual(login.navigations(), [(LOGIN_URL, login.session)])
        self.assertEqual(login.closed_targets(), [login.target])
        self.assertEqual(probe.closed_targets(), [probe.target])

    def test_probe_target_close_failure_still_closes_both_connections(self):
        login = FakeLoginCDP("login", self.clock, [SEARCH_URL])
        probe = FakeLoginCDP(
            "probe", self.clock, failures={"Target.closeTarget": RuntimeError("already closed")}
        )
        available = boss.LoginProbeResult(boss.LoginProbeStatus.AVAILABLE)
        result, _connect, check = self.run_wait(login, probe, [available])

        self.assertTrue(result)
        check.assert_called_once()
        self.assertTrue(login.closed)
        self.assertTrue(probe.closed)
        self.assertEqual(login.closed_targets(), [login.target])
        self.assertEqual(probe.closed_targets(), [probe.target])

    def test_probe_attachment_failure_cleans_target_and_both_connections(self):
        login = FakeLoginCDP("login", self.clock, [SEARCH_URL])
        probe = FakeLoginCDP(
            "probe", self.clock, failures={"Target.attachToTarget": RuntimeError("attach failed")}
        )
        with (
            patch.object(boss, "CDPSession", side_effect=[login, probe]),
            patch.object(boss.time, "time", side_effect=self.clock.time),
            patch.object(boss.time, "sleep", side_effect=self.clock.sleep),
            patch.object(boss, "probe_login_state") as check,
            redirect_stdout(StringIO()),
            self.assertRaisesRegex(RuntimeError, "attach failed"),
        ):
            boss.wait_for_login(timeout=20, interval=3)

        check.assert_not_called()
        self.assertEqual(login.closed_targets(), [])
        self.assertEqual(probe.closed_targets(), [probe.target])
        self.assertTrue(login.closed)
        self.assertTrue(probe.closed)


if __name__ == "__main__":
    unittest.main()
