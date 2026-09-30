"""起動済みの在庫APIにHTTPリクエストを送り、statusとJSONを照合する。"""

import argparse
import json
from urllib.error import HTTPError
from urllib.request import ProxyHandler, Request, build_opener


OPENER = build_opener(ProxyHandler({}))


def request(base, method, path, payload=None, raw=None):
    data = raw
    if payload is not None:
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    req = Request(base + path, data=data, method=method)
    if data is not None:
        req.add_header("Content-Type", "application/json")
    try:
        response = OPENER.open(req, timeout=5)
    except HTTPError as exc:
        response = exc
    with response:
        body = response.read()
        return response.status, json.loads(body) if body else None


def exercise(base):
    passed = []

    def expect(label, method, path, status=200, body=None, payload=None, raw=None):
        actual_status, actual_body = request(base, method, path, payload, raw)
        if actual_status != status or (body is not None and actual_body != body):
            raise AssertionError(f"{label}: got {actual_status} {actual_body!r}")
        passed.append(label)
        return actual_body

    empty = {"item_count": 0, "total_quantity": 0, "out_of_stock": 0}
    expect("empty list", "GET", "/items", body=[])
    expect("empty summary", "GET", "/inventory/summary", body=empty)
    expect("missing item", "GET", "/items/999", status=404)
    for method in ("GET", "PUT", "DELETE"):
        expect(f"{method} invalid id", method, "/items/0", status=400,
               payload={"name": "test", "quantity": 1} if method == "PUT" else None)

    bad_payloads = [
        {"name": "", "quantity": 1},
        {"name": "あ" * 41, "quantity": 1},
        {"name": "test", "quantity": -1},
        {"name": "test", "quantity": 1000001},
        {"name": "test", "quantity": 2**40},
        {"name": "test", "quantity": "1"},
        {"name": "test"},
        {"name": "test", "quantity": 1, "extra": True},
    ]
    for index, payload in enumerate(bad_payloads):
        expect(f"invalid payload {index}", "POST", "/items", status=400, payload=payload)
    expect("malformed JSON", "POST", "/items", status=400, raw=b'{"name":')
    expect("failed writes leave DB empty", "GET", "/inventory/summary", body=empty)

    item = {"id": 1, "name": "あ" * 40, "quantity": 1000000}
    expect("valid upper boundaries", "POST", "/items", body=item,
           payload={"name": item["name"], "quantity": item["quantity"]})
    expect("read item", "GET", "/items/1", body=item)
    expect("invalid update", "PUT", "/items/1", status=400,
           payload={"name": "", "quantity": 0})
    expect("invalid update preserves item", "GET", "/items/1", body=item)
    updated = {"id": 1, "name": "ノート", "quantity": 0}
    expect("update to zero stock", "PUT", "/items/1", body=updated,
           payload={"name": "ノート", "quantity": 0})
    expect("missing update", "PUT", "/items/999", status=404,
           payload={"name": "test", "quantity": 1})
    attack_name = "x'); DROP TABLE sample_inventory_items;--"
    second = {"id": 2, "name": attack_name, "quantity": 12}
    expect("SQL values are bound", "POST", "/items", body=second,
           payload={"name": attack_name, "quantity": 12})
    expect("list order", "GET", "/items", body=[second, updated])
    summary = {"item_count": 2, "total_quantity": 12, "out_of_stock": 1}
    expect("summary", "GET", "/inventory/summary", body=summary)

    expect("delete", "DELETE", "/items/1", body={"id": 1, "deleted": True})
    expect("repeat delete", "DELETE", "/items/1", body={"id": 1, "deleted": False})
    expect("read deleted item", "GET", "/items/1", status=404)
    expect("summary after delete", "GET", "/inventory/summary",
           body={"item_count": 1, "total_quantity": 12, "out_of_stock": 0})

    for index in range(100):
        expect(f"populate item {index}", "POST", "/items",
               payload={"name": f"item-{index}", "quantity": 1})
    rows = expect("bounded list", "GET", "/items")
    if len(rows) != 100 or rows[0]["name"] != "item-99" or rows[-1]["name"] != "item-0":
        raise AssertionError("expected latest 100 items in descending ID order")
    expect("summary includes rows outside list", "GET", "/inventory/summary",
           body={"item_count": 101, "total_quantity": 112, "out_of_stock": 0})
    print(f"PASS: {len(passed)} HTTP checks, including validation, CRUD, summary and list limit")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://127.0.0.1:8090",
                        help="起動済みサーバーのURL（空のDBを使用）")
    exercise(parser.parse_args().base_url.rstrip("/"))


if __name__ == "__main__":
    main()
