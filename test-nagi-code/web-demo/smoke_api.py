"""起動済みのNagi Tasksにリクエストを送り、応答を照合する。"""
import argparse
import json
from urllib.error import HTTPError
from urllib.request import Request, ProxyHandler, build_opener

opener = build_opener(ProxyHandler({}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://127.0.0.1:8091")
    base = parser.parse_args().base_url.rstrip("/")
    created = []
    passed = []

    def request(method, path, payload=None, raw=None):
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8") if payload is not None else raw
        req = Request(base + path, data=data, method=method)
        if data is not None:
            req.add_header("Content-Type", "application/json")
        try:
            response = opener.open(req, timeout=5)
        except HTTPError as error:
            response = error
        with response:
            content = response.read().decode("utf-8")
            return response.status, response.headers, content

    def expect(label, method, path, status=200, body=None, payload=None, raw=None):
        code, headers, content = request(method, path, payload, raw)
        if code != status:
            raise AssertionError(f"{label}: {code} {content}")
        if content and headers.get_content_type() == "application/json":
            result = json.loads(content)
        else:
            result = content
        if body is not None and result != body:
            raise AssertionError(f"{label}: {result!r} != {body!r}")
        passed.append(label)
        return result

    try:
        code, headers, page = request("GET", "/")
        assert code == 200 and headers.get_content_type() == "text/plain" and page == "Task JSON API is available at /api/tasks."
        passed.append("plain-text API landing route")
        baseline = expect("initial stats", "GET", "/api/stats")
        task = expect("create", "POST", "/api/tasks", payload={"title": "APIテスト専用", "done": False})
        created.append(task["id"])
        assert task["title"] == "APIテスト専用" and task["done"] is False
        task_path = f'/api/tasks/{task["id"]}'
        expect("read", "GET", task_path, body=task)
        updated = {"id": task["id"], "title": "APIテスト更新済み", "done": True}
        expect("update", "PUT", task_path, body=updated, payload={"title": updated["title"], "done": True})
        expect("aggregate", "GET", "/api/stats", body={"total": baseline["total"] + 1, "completed": baseline["completed"] + 1})
        for index, payload in enumerate([
            {"title": "", "done": False}, {"title": "あ" * 81, "done": False},
            {"title": "test", "done": 1}, {"title": "test"},
            {"title": "test", "done": False, "extra": 1},
        ]):
            expect(f"invalid input {index}", "POST", "/api/tasks", status=400, payload=payload)
        expect("malformed JSON", "POST", "/api/tasks", status=400, raw=b'{"title":')
        expect("invalid update", "PUT", task_path, status=400, payload={"title": "", "done": False})
        expect("failed update preserves data", "GET", task_path, body=updated)
        expect("invalid id", "GET", "/api/tasks/0", status=400)
        edge = expect("UTF-8 boundary", "POST", "/api/tasks", payload={"title": "あ" * 80, "done": False})
        created.append(edge["id"])
        title = "x'); DROP TABLE demo_tasks;--"
        bound = expect("bound SQL value", "POST", "/api/tasks", payload={"title": title, "done": False})
        created.append(bound["id"])
        assert bound["title"] == title
        expect("delete", "DELETE", task_path, body=1)
        expect("repeat delete", "DELETE", task_path, body=0)
        expect("deleted item is missing", "GET", task_path, status=404)
    finally:
        for identifier in created:
            request("DELETE", f"/api/tasks/{identifier}")
    expect("test rows cleaned through API", "GET", "/api/stats", body=baseline)
    print(f"PASS: {len(passed)} HTTP checks")


if __name__ == "__main__":
    main()
