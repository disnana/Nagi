"""起動済みのResult分岐サンプルへHTTPリクエストを送り、応答を照合する。"""
import argparse
import json
from urllib.error import HTTPError
from urllib.request import ProxyHandler, build_opener


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://127.0.0.1:8097")
    base = parser.parse_args().base_url.rstrip("/")
    opener = build_opener(ProxyHandler({}))
    cases = [
        ("/api/double?value=21", 200, {"value": 42}),
        ("/api/double?value=0", 200, {"value": 0}),
        ("/api/double?value=1000000", 200, {"value": 2000000}),
        ("/api/double?value=abc", 400, {"error": "value must be an integer"}),
        ("/api/double?value=9223372036854775808", 400, {"error": "value must be an integer"}),
        ("/api/double?value=-1", 400, {"error": "value must be between 0 and 1000000"}),
        ("/api/double?value=1000001", 400, {"error": "value must be between 0 and 1000000"}),
        ("/api/items/1", 200, {"id": 1, "name": "notebook"}),
        ("/api/items/2", 404, {"error": "not found"}),
        ("/api/items/0", 400, {"error": "id must be positive"}),
        ("/api/items/1000001", 404, {"error": "item is outside the sample range"}),
        ("/api/fallback", 200, {"id": 0, "name": "cached item"}),
        ("/api/db-error", 500, {"error": "internal error"}),
        ("/api/internal-error", 500, {"error": "internal error"}),
        # DB失敗後も通常のリクエストを処理できる。
        ("/api/items/1", 200, {"id": 1, "name": "notebook"}),
    ]
    for path, status, expected in cases:
        try:
            response = opener.open(base + path, timeout=5)
        except HTTPError as error:
            response = error
        with response:
            body = response.read().decode("utf-8")
            assert response.status == status, (path, response.status, body)
            assert response.headers.get_content_type() == "application/json", path
            assert json.loads(body) == expected, (path, body, expected)
    print(f"PASS: {len(cases)} HTTP checks (Result branches, 400/404/500, fallback, recovery)")


if __name__ == "__main__":
    main()
