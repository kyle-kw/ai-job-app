from __future__ import annotations

import json
import os
import pathlib
import subprocess
import unittest
from contextlib import redirect_stdout
from io import StringIO
from unittest.mock import patch

import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

import worker  # noqa: E402


class WorkerTests(unittest.TestCase):
    def test_text_extraction_does_not_invent_experience(self):
        profile = worker.profile_from_text(
            "林知远\nAI 应用研发工程师\nlin@example.com\n熟悉 Python、FastAPI、RAG 和 Docker。",
            "resume.txt",
        )
        self.assertEqual(profile["name"], "林知远")
        self.assertIn("Python", profile["skills"])
        self.assertEqual(profile["experiences"], [])

    def test_rendercv_yaml_is_structured(self):
        profile = worker.base_profile(
            "resume.yaml", "林知远", "lin@example.com", "", "上海", "", "AI 工程师", "简介", ["Python"], [], []
        )
        data = worker.profile_to_rendercv(profile)
        self.assertEqual(data["cv"]["name"], "林知远")
        self.assertEqual(data["cv"]["sections"]["核心技能"], ["Python"])

    def test_rendercv_yaml_keeps_education_separate(self):
        import yaml

        path = ROOT / "tests" / "fixtures" / "sample_resume.yaml"
        profile = worker.profile_from_yaml(yaml.safe_load(path.read_text(encoding="utf-8")), path.name)
        self.assertEqual(len(profile["experiences"]), 1)
        self.assertEqual(len(profile["education"]), 1)
        self.assertEqual(profile["education"][0]["institution"], "浙江工业大学")

    def test_market_report_stays_scoped(self):
        report = worker.market_report([{"skills": ["Python"], "experience": "3-5年", "degree": "本科"}], "AI Agent", "上海")
        self.assertIn("本次整理", report)
        self.assertIn("上海", report)

    def test_shanghai_timezone_is_explicit(self):
        self.assertTrue(worker.now().endswith("+08:00"))

    def test_scrape_resolves_city_code_and_connects_boss(self):
        class FakeBoss:
            setup_called = False
            received_city = ""

            @staticmethod
            def resolve_city(city):
                self.assertEqual(city, "上海")
                return "上海", "101020100"

            @classmethod
            def run_setup_chrome(cls, *_args, **_kwargs):
                cls.setup_called = True
                return 0

            @classmethod
            def scrape_list(cls, _keyword, city, _pages, _filters, _output, **_kwargs):
                cls.received_city = city
                return {
                    "jobs": [{
                        "job_id": "job-1",
                        "title": "AI 工程师",
                        "boss_name": "示例公司",
                        "location": "上海·浦东新区",
                        "salary": "20-30K",
                    }]
                }

            @staticmethod
            def scrape_details(*_args, **_kwargs):
                return []

        fake = FakeBoss()
        with patch.object(worker, "load_boss_module", return_value=fake), redirect_stdout(StringIO()):
            result = worker.scrape_jobs({"keyword": "AI Agent", "city": " 上海 ", "pages": 1})

        self.assertTrue(fake.setup_called)
        self.assertEqual(fake.received_city, "101020100")
        self.assertEqual(result["resolvedCity"], "上海")
        self.assertEqual(result["cityCode"], "101020100")
        self.assertEqual(len(result["jobs"]), 1)

    def test_unknown_city_is_rejected_before_browser_connection(self):
        class FakeBoss:
            @staticmethod
            def resolve_city(city):
                return city, city

        with patch.object(worker, "load_boss_module", return_value=FakeBoss()):
            with self.assertRaisesRegex(RuntimeError, "无法识别城市"):
                worker.scrape_jobs({"keyword": "AI", "city": "乱码城市", "pages": 1})

    def test_jsonl_process_boundary_forces_utf8(self):
        fixture = ROOT / "tests" / "fixtures" / "sample_resume.yaml"
        request = {
            "op": "extract_resume",
            "params": {"path": str(fixture), "fileName": "中文简历.yaml"},
        }
        env = os.environ.copy()
        env["PYTHONUTF8"] = "0"
        env.pop("PYTHONIOENCODING", None)
        process = subprocess.run(
            [sys.executable, str(ROOT / "worker.py")],
            input=(json.dumps(request, ensure_ascii=False) + "\n").encode("utf-8"),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            timeout=15,
            check=False,
        )
        self.assertEqual(process.returncode, 0, process.stderr.decode("utf-8"))
        messages = [json.loads(line) for line in process.stdout.decode("utf-8").splitlines()]
        result = next(message for message in messages if message.get("type") == "result")
        self.assertEqual(result["data"]["profile"]["name"], "林知远")
        self.assertEqual(result["data"]["profile"]["location"], "上海")
        self.assertEqual(result["data"]["profile"]["sourceFileName"], "中文简历.yaml")


if __name__ == "__main__":
    unittest.main()
