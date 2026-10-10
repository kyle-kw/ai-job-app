from __future__ import annotations

import base64
import json
import pathlib
import random
import sys
import tempfile
import types
import unittest
from contextlib import redirect_stdout
from io import StringIO
from unittest.mock import Mock, patch

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from vendor import boss_cdp_raw as boss


class FakeClock:
    def __init__(self):
        self.current = 0.0
        self.sleeps = []

    def time(self):
        return self.current

    def sleep(self, duration):
        self.sleeps.append(duration)
        self.current += duration


def joblist_events(request_id, session="page-session", status=200):
    url = "https://www.zhipin.com/wapi/zpgeek/search/joblist.json?_=1"
    return [
        {
            "sessionId": session,
            "method": "Network.requestWillBeSent",
            "params": {"requestId": request_id, "request": {"url": url}},
        },
        {
            "sessionId": session,
            "method": "Network.responseReceived",
            "params": {"requestId": request_id, "response": {"url": url, "status": status}},
        },
        {
            "sessionId": session,
            "method": "Network.loadingFinished",
            "params": {"requestId": request_id},
        },
    ]


def job_page(*identifiers, more=False):
    return {
        "code": 0,
        "zpData": {
            "hasMore": more,
            "jobList": [
                {
                    "jobName": f"岗位 {identifier}",
                    "salaryDesc": "20-40K",
                    "encryptJobId": identifier,
                    "brandName": "示例公司",
                }
                for identifier in identifiers
            ],
        },
    }


class FakeCaptureCDP:
    def __init__(self, batches=(), responses=None, clock=None, failures=None):
        self.events = []
        self.pending = list(batches)
        self.responses = dict(responses or {})
        self.clock = clock or FakeClock()
        self.failures = dict(failures or {})
        self.sent = []
        self.drains = 0
        self.closed = False

    def send(self, method, params=None, sid=None, **_kwargs):
        params = params or {}
        self.sent.append((method, params, sid))
        if method in self.failures:
            raise self.failures[method]
        if method == "Target.createTarget":
            return {"result": {"targetId": "page-target"}}
        if method == "Target.attachToTarget":
            return {"result": {"sessionId": "page-session"}}
        if method == "Network.getResponseBody":
            response = self.responses[params["requestId"]]
            if isinstance(response, Exception):
                raise response
            body = response if isinstance(response, str) else json.dumps(response)
            return {"result": {"body": body, "base64Encoded": False}}
        return {"result": {}}

    def drain_events(self, duration):
        self.drains += 1
        self.clock.current += max(duration, 0.001)
        if self.pending:
            self.events.extend(self.pending.pop(0))

    def eval_js(self, expression, sid=None):
        self.sent.append(("Runtime.evaluate", {"expression": expression}, sid))
        return None

    def close(self):
        self.closed = True


class BossCaptureTests(unittest.TestCase):
    def setUp(self):
        self.previous_budget = boss._request_counter
        boss._request_counter = 0

    def tearDown(self):
        boss._request_counter = self.previous_budget

    def run_scrape(self, cdp, output, pages=3, on_job=None):
        with (
            patch.object(boss, "CDPSession", return_value=cdp),
            patch.object(boss.time, "time", side_effect=cdp.clock.time),
            patch.object(boss.time, "sleep", side_effect=cdp.clock.sleep),
            patch.object(boss, "random", random.Random(7)),
            redirect_stdout(StringIO()),
        ):
            return boss.scrape_list("AI Agent", "上海", pages, {}, output, on_job=on_job)

    def test_cdp_send_preserves_network_events_arriving_before_command_response(self):
        events = joblist_events("during-command")
        socket = Mock()
        socket.recv.side_effect = [
            "invalid json",
            *map(json.dumps, events),
            json.dumps({"id": 1, "result": {"ok": True}}),
        ]
        session = boss.CDPSession.__new__(boss.CDPSession)
        session.mid = 0
        session.ws = socket
        session.events = []

        result = session.send("Network.enable", {}, "page-session")

        self.assertEqual(result["result"], {"ok": True})
        self.assertEqual(session.events, events)
        self.assertEqual(json.loads(socket.send.call_args.args[0])["sessionId"], "page-session")

    def test_cdp_drain_keeps_events_and_restores_socket_timeout_when_receiving_fails(self):
        socket = Mock()
        socket.recv.side_effect = [
            json.dumps(joblist_events("request")[0]),
            OSError("connection gone"),
        ]
        session = boss.CDPSession.__new__(boss.CDPSession)
        session.ws = socket
        session.events = []

        with (
            patch.object(
                boss, "websocket", types.SimpleNamespace(WebSocketTimeoutException=TimeoutError)
            ),
            self.assertRaisesRegex(OSError, "connection gone"),
        ):
            session.drain_events(1)

        self.assertEqual(len(session.events), 1)
        self.assertEqual(socket.settimeout.call_args.args, (60,))

    def test_capture_ignores_old_events_and_isolates_same_request_id_across_sessions(self):
        cdp = FakeCaptureCDP(responses={"current": job_page("correct")})
        cdp.events.extend(joblist_events("stale"))
        capture = boss.NetworkJoblistCapture(cdp, "page-session")
        local = joblist_events("current")
        foreign = joblist_events("current", session="other-session")
        cdp.pending = [foreign, local[:2], foreign[-1:] + local[-1:]]
        with patch.object(boss.time, "time", side_effect=cdp.clock.time):
            result = capture.wait_next_response(timeout=3)

        self.assertEqual(result, job_page("correct"))
        self.assertEqual(cdp.drains, 3)
        bodies = [
            params["requestId"]
            for method, params, _sid in cdp.sent
            if method == "Network.getResponseBody"
        ]
        self.assertEqual(bodies, ["current"])

    def test_capture_decodes_base64_keeps_http_status_and_consumes_each_response_once(self):
        cdp = FakeCaptureCDP(batches=[joblist_events("request", status=200)])
        payload = {"code": 37, "message": "您的环境存在异常"}
        encoded = base64.b64encode(json.dumps(payload).encode()).decode()
        original_send = cdp.send

        def send(method, params=None, sid=None):
            if method == "Network.getResponseBody":
                cdp.sent.append((method, params, sid))
                return {"result": {"body": encoded, "base64Encoded": True}}
            return original_send(method, params, sid)

        cdp.send = send
        capture = boss.NetworkJoblistCapture(cdp, "page-session")
        with patch.object(boss.time, "time", side_effect=cdp.clock.time):
            self.assertEqual(capture.wait_next_response(timeout=1), payload)
            self.assertEqual(capture.last_http_status, 200)
            self.assertIsNone(capture.wait_next_response(timeout=0.1))
        self.assertEqual(
            sum(method == "Network.getResponseBody" for method, _params, _sid in cdp.sent), 1
        )

    def test_capture_skips_unavailable_or_invalid_bodies_and_accepts_later_response(self):
        cdp = FakeCaptureCDP(
            batches=[joblist_events("gone"), joblist_events("invalid"), joblist_events("valid")],
            responses={
                "gone": TimeoutError("body evicted"),
                "invalid": "not JSON",
                "valid": job_page("valid"),
            },
        )
        capture = boss.NetworkJoblistCapture(cdp, "page-session")
        with patch.object(boss.time, "time", side_effect=cdp.clock.time):
            self.assertEqual(capture.wait_next_response(timeout=3), job_page("valid"))

    def test_http_restriction_is_preserved_even_when_response_body_is_html_or_unavailable(self):
        for body in ("<html>访问频繁</html>", TimeoutError("body unavailable")):
            with self.subTest(body=body):
                cdp = FakeCaptureCDP(
                    batches=[joblist_events("restricted", status=429)],
                    responses={"restricted": body},
                )
                with patch.object(boss.time, "time", side_effect=cdp.clock.time):
                    result = boss.probe_login_state(cdp, "page-session", timeout=1)
                self.assertIs(result.status, boss.LoginProbeStatus.RESTRICTED)
                self.assertIn("HTTP 429", result.message)
                self.assertFalse(
                    any(method == "Runtime.evaluate" for method, _params, _sid in cdp.sent)
                )

    def test_capture_timeout_classifies_security_redirect_login_redirect_or_transport_failure(self):
        cases = [
            (
                {"url": "https://www.zhipin.com/web/common/security-check?_security_check=1"},
                boss.LoginProbeStatus.RESTRICTED,
            ),
            ({"url": "https://www.zhipin.com/web/user/"}, boss.LoginProbeStatus.UNAUTHENTICATED),
            (
                {"url": "https://www.zhipin.com/web/geek/job", "page_text": "请完成验证"},
                boss.LoginProbeStatus.RESTRICTED,
            ),
            ({"url": "about:blank", "page_text": ""}, boss.LoginProbeStatus.RESPONSE_ERROR),
        ]
        for page, status in cases:
            with self.subTest(page=page):
                boss._request_counter = 0
                cdp = FakeCaptureCDP()
                with (
                    patch.object(cdp, "eval_js", return_value=json.dumps(page)) as diagnostic,
                    patch.object(boss.time, "time", side_effect=cdp.clock.time),
                ):
                    result = boss.probe_login_state(cdp, "page-session", timeout=0.1)
                self.assertIs(result.status, status)
                self.assertEqual(boss._request_counter, 1)
                self.assertEqual(
                    sum(method == "Page.navigate" for method, _params, _sid in cdp.sent), 1
                )
                self.assertNotIn("XMLHttpRequest", diagnostic.call_args.args[0])
                self.assertNotIn("fetch(", diagnostic.call_args.args[0])

    def test_background_target_has_focus_and_visibility_before_navigation(self):
        cdp = FakeCaptureCDP()
        target, session = boss.create_page_session(cdp)
        cdp.send("Page.navigate", {"url": "https://www.zhipin.com"}, session)
        methods = [method for method, _params, _sid in cdp.sent]
        self.assertEqual((target, session), ("page-target", "page-session"))
        self.assertTrue(cdp.sent[0][1]["background"])
        self.assertLess(
            methods.index("Emulation.setFocusEmulationEnabled"), methods.index("Page.navigate")
        )
        self.assertLess(
            methods.index("Page.addScriptToEvaluateOnNewDocument"), methods.index("Page.navigate")
        )
        self.assertNotIn("Target.activateTarget", methods)

    def test_interactive_login_target_is_foreground_and_has_no_background_override(self):
        cdp = FakeCaptureCDP()
        boss.create_page_session(cdp, background=False)
        self.assertFalse(cdp.sent[0][1]["background"])
        methods = [method for method, _params, _sid in cdp.sent]
        self.assertNotIn("Emulation.setFocusEmulationEnabled", methods)
        self.assertNotIn("Page.addScriptToEvaluateOnNewDocument", methods)

    def test_api_mapping_preserves_plain_salary_security_context_and_separate_activity(self):
        mapped = boss.map_api_job(
            {
                "jobName": "AI 工程师",
                "salaryDesc": "20-40K·16薪",
                "cityName": "上海",
                "areaDistrict": "浦东新区",
                "businessDistrict": "张江",
                "jobExperience": "3-5年",
                "jobDegree": "本科",
                "brandName": "示例公司",
                "bossTitle": "技术总监",
                "bossOnline": True,
                "skills": ["Python", "Docker"],
                "welfareList": ["五险一金"],
                "encryptJobId": "encrypted-job",
                "encryptBossId": "encrypted-boss",
                "securityId": "secure-id",
                "lid": "list-id",
            }
        )
        self.assertEqual(mapped["salary"], "20-40K·16薪")
        self.assertEqual(mapped["salary_source"], "api")
        self.assertEqual(mapped["location"], "上海·浦东新区·张江")
        self.assertEqual(mapped["tags"], "3-5年 | 本科")
        self.assertEqual(mapped["skills"], "Python | Docker")
        self.assertEqual(mapped["welfare"], "五险一金")
        self.assertEqual(mapped["boss_name"], "示例公司")
        self.assertEqual(mapped["boss_active_status"], "在线")
        self.assertEqual(mapped["security_id"], "secure-id")
        self.assertEqual(mapped["lid"], "list-id")
        self.assertEqual(mapped["job_link"], "https://www.zhipin.com/job_detail/encrypted-job.html")
        self.assertEqual(boss.map_api_jobs({"code": 0, "zpData": {"jobList": [None, "bad"]}}), [])

    def test_recruiter_activity_is_separate_from_description_and_company_introduction(self):
        description = "负责人工智能产品的研发、质量保障和上线复盘。\n" * 8
        page = (
            f"职位描述\n{description}张女士\n今日活跃\n示例公司\n·\n招聘者\n"
            "竞争力分析\n公司介绍\n专注于企业人工智能产品。\n更多职位"
        )
        fields = boss.extract_detail_fields({"jd": page, "page_text": page})
        self.assertEqual(fields["boss_active_status"], "今日活跃")
        self.assertEqual(
            fields["jd"], description.strip() + "\n\n公司介绍\n专注于企业人工智能产品。"
        )
        record = boss.build_detail_record({"boss_active_status": "在线"}, fields)
        self.assertEqual(record["boss_active_status"], "今日活跃")
        self.assertNotIn("今日活跃", record["jd"])
        fallback = boss.build_detail_record({"boss_active_status": "在线"}, {"jd": description})
        self.assertEqual(fallback["boss_active_status"], "在线")

    def test_list_uses_one_navigation_scrolls_for_next_page_and_streams_unique_jobs(self):
        cdp = FakeCaptureCDP(
            batches=[joblist_events("page-1"), joblist_events("page-2")],
            responses={
                "page-1": job_page("first", "duplicate", more=True),
                "page-2": job_page("duplicate", "last"),
            },
        )
        streamed = []
        with tempfile.TemporaryDirectory() as temporary:
            output = str(pathlib.Path(temporary) / "jobs.json")
            result = self.run_scrape(cdp, output, on_job=streamed.append)
            saved = json.loads(pathlib.Path(output).read_text(encoding="utf-8"))

        self.assertEqual(result["total"], 3)
        self.assertEqual(
            [job["job_id"] for job in streamed], [job["job_id"] for job in result["jobs"]]
        )
        self.assertEqual(saved["total"], 3)
        methods = [method for method, _params, _sid in cdp.sent]
        self.assertEqual(methods.count("Page.navigate"), 1)
        self.assertEqual(boss._request_counter, 2)
        scripts = [
            params["expression"]
            for method, params, _sid in cdp.sent
            if method == "Runtime.evaluate"
        ]
        self.assertTrue(any("scrollTo" in script for script in scripts))
        self.assertFalse(
            any("XMLHttpRequest" in script or "fetch(" in script for script in scripts)
        )
        self.assertTrue(cdp.closed)

    def test_has_more_false_stops_even_when_final_list_is_empty(self):
        cdp = FakeCaptureCDP(
            batches=[joblist_events("page-1"), joblist_events("empty-final")],
            responses={"page-1": job_page("first", more=True), "empty-final": job_page()},
        )
        with tempfile.TemporaryDirectory() as temporary:
            result = self.run_scrape(cdp, str(pathlib.Path(temporary) / "jobs.json"), pages=5)
        self.assertEqual(result["total"], 1)
        self.assertEqual(boss._request_counter, 2)

    def test_login_or_restriction_on_later_page_stops_without_more_scrolls(self):
        for response, http_status in [
            ({"code": 37, "message": "您的环境存在异常"}, 200),
            ({"code": 0, "zpData": {"jobList": [{"salaryDesc": ""}]}}, 200),
            ({"code": 0, "zpData": {"jobList": []}}, 429),
            ({"code": 7, "message": "业务异常"}, 200),
        ]:
            with self.subTest(response=response, http_status=http_status):
                boss._request_counter = 0
                cdp = FakeCaptureCDP(
                    batches=[joblist_events("ok"), joblist_events("blocked", status=http_status)],
                    responses={"ok": job_page("first", more=True), "blocked": response},
                )
                with tempfile.TemporaryDirectory() as temporary:
                    output = str(pathlib.Path(temporary) / "jobs.json")
                    with self.assertRaises(boss.LoginGateError):
                        self.run_scrape(cdp, output, pages=5)
                    self.assertEqual(
                        json.loads(pathlib.Path(output).read_text(encoding="utf-8"))["total"], 1
                    )
                self.assertEqual(boss._request_counter, 2)
                self.assertTrue(cdp.closed)

    def test_budget_exhaustion_prevents_navigation_or_next_page_scroll(self):
        for allowed in (0, 1):
            with self.subTest(allowed=allowed):
                boss._request_counter = 0
                cdp = FakeCaptureCDP(
                    batches=[joblist_events("first")],
                    responses={"first": job_page("first", more=True)},
                )
                with (
                    tempfile.TemporaryDirectory() as temporary,
                    patch.object(boss, "MAX_API_REQUESTS", allowed),
                ):
                    with self.assertRaisesRegex(RuntimeError, "最大请求数"):
                        self.run_scrape(cdp, str(pathlib.Path(temporary) / "jobs.json"))
                navigations = sum(method == "Page.navigate" for method, _params, _sid in cdp.sent)
                self.assertEqual(navigations, allowed)
                self.assertFalse(
                    any(
                        "scrollTo" in params.get("expression", "")
                        for _method, params, _sid in cdp.sent
                    )
                )
                self.assertTrue(cdp.closed)

    def test_capture_enable_timeout_and_target_close_failure_still_close_connection(self):
        cdp = FakeCaptureCDP(
            failures={
                "Network.enable": TimeoutError("enable timed out"),
                "Target.closeTarget": RuntimeError("already detached"),
            }
        )
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(RuntimeError, "CDP 连接异常"):
                self.run_scrape(cdp, str(pathlib.Path(temporary) / "jobs.json"))
        self.assertTrue(cdp.closed)
        self.assertEqual(
            sum(method == "Target.closeTarget" for method, _params, _sid in cdp.sent), 1
        )

    def test_page_attach_failure_closes_created_target_and_cdp_connection(self):
        cdp = FakeCaptureCDP(failures={"Target.attachToTarget": RuntimeError("attach failed")})
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(RuntimeError, "attach failed"):
                self.run_scrape(cdp, str(pathlib.Path(temporary) / "jobs.json"))
        self.assertTrue(cdp.closed)
        self.assertIn(("Target.closeTarget", {"targetId": "page-target"}, None), cdp.sent)

    def test_atomic_write_failure_keeps_previous_json_and_removes_temporary_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = pathlib.Path(temporary) / "jobs.json"
            previous = {"jobs": [{"job_id": "saved", "title": "已保存岗位"}], "total": 1}
            output.write_text(json.dumps(previous, ensure_ascii=False), encoding="utf-8")
            previous_bytes = output.read_bytes()
            with (
                patch.object(boss.os, "replace", side_effect=OSError("file locked")),
                self.assertRaisesRegex(OSError, "file locked"),
            ):
                boss._atomic_write_json(str(output), {"jobs": [], "total": 0})

            self.assertEqual(output.read_bytes(), previous_bytes)
            self.assertEqual(json.loads(output.read_text(encoding="utf-8")), previous)
            self.assertEqual(list(output.parent.iterdir()), [output])

    def test_login_rotates_sample_searches_and_backs_off_before_success(self):
        cdp = FakeCaptureCDP()
        results = [
            boss.LoginProbeResult(boss.LoginProbeStatus.EMPTY),
            boss.LoginProbeResult(boss.LoginProbeStatus.UNAUTHENTICATED),
            boss.LoginProbeResult(boss.LoginProbeStatus.AVAILABLE),
        ]
        with (
            patch.object(boss, "CDPSession", return_value=cdp),
            patch.object(boss.time, "time", side_effect=cdp.clock.time),
            patch.object(boss.time, "sleep", side_effect=cdp.clock.sleep),
            patch.object(boss, "probe_login_state", side_effect=results) as probe,
            redirect_stdout(StringIO()),
        ):
            self.assertTrue(boss.wait_for_login(timeout=100, interval=3))
        self.assertEqual(cdp.clock.sleeps, [3, 6])
        self.assertEqual(
            [(call.kwargs["query"], call.kwargs["city_code"]) for call in probe.call_args_list],
            list(boss.LOGIN_PROBE_TARGETS[:3]),
        )
        self.assertFalse(cdp.sent[0][1]["background"])
        self.assertTrue(cdp.closed)

    def test_login_timeout_bounds_wait_and_closes_connection(self):
        cdp = FakeCaptureCDP()
        with (
            patch.object(boss, "CDPSession", return_value=cdp),
            patch.object(boss.time, "time", side_effect=cdp.clock.time),
            patch.object(boss.time, "sleep", side_effect=cdp.clock.sleep),
            patch.object(
                boss,
                "probe_login_state",
                return_value=boss.LoginProbeResult(boss.LoginProbeStatus.UNAUTHENTICATED),
            ) as probe,
            redirect_stdout(StringIO()),
        ):
            self.assertFalse(boss.wait_for_login(timeout=5, interval=3))
        self.assertEqual(cdp.clock.sleeps, [3, 2])
        self.assertEqual([call.kwargs["timeout"] for call in probe.call_args_list], [5, 2])
        self.assertTrue(cdp.closed)

    def test_login_cdp_timeouts_retry_with_limit_and_restrictions_never_retry(self):
        for outcome, expected_probes in [
            (TimeoutError("CDP recv timed out"), boss.LOGIN_PROBE_MAX_TRANSIENT_ERRORS + 1),
            (boss.LoginProbeResult(boss.LoginProbeStatus.RESTRICTED, code=37), 1),
        ]:
            with self.subTest(outcome=outcome):
                cdp = FakeCaptureCDP()
                with (
                    patch.object(boss, "CDPSession", return_value=cdp),
                    patch.object(boss.time, "time", side_effect=cdp.clock.time),
                    patch.object(boss.time, "sleep", side_effect=cdp.clock.sleep),
                    patch.object(
                        boss,
                        "probe_login_state",
                        side_effect=(outcome if isinstance(outcome, Exception) else None),
                        return_value=outcome,
                    ) as probe,
                    redirect_stdout(StringIO()),
                ):
                    self.assertFalse(boss.wait_for_login(timeout=300, interval=3))
                self.assertEqual(probe.call_count, expected_probes)
                self.assertEqual(len(cdp.clock.sleeps), expected_probes - 1)
                self.assertTrue(cdp.closed)

    def test_successful_login_target_close_failure_still_closes_connection(self):
        cdp = FakeCaptureCDP(failures={"Target.closeTarget": RuntimeError("already closed")})
        with (
            patch.object(boss, "CDPSession", return_value=cdp),
            patch.object(
                boss,
                "probe_login_state",
                return_value=boss.LoginProbeResult(boss.LoginProbeStatus.AVAILABLE),
            ),
            redirect_stdout(StringIO()),
        ):
            self.assertTrue(boss.wait_for_login(timeout=5))
        self.assertTrue(cdp.closed)


if __name__ == "__main__":
    unittest.main()
