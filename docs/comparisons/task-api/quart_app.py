"""Quart implementation used by the Nagi task API comparison."""

import asyncio
import os
import sqlite3
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import Any

from quart import Quart, abort, jsonify, request


MAX_I64 = 9_223_372_036_854_775_807


@contextmanager
def connect(database: str) -> Iterator[sqlite3.Connection]:
    connection = sqlite3.connect(database)
    connection.row_factory = sqlite3.Row
    try:
        with connection:
            yield connection
    finally:
        connection.close()


def initialize(database: str) -> None:
    Path(database).parent.mkdir(parents=True, exist_ok=True)
    with connect(database) as connection:
        connection.execute(
            """CREATE TABLE IF NOT EXISTS tasks(
                   id INTEGER PRIMARY KEY,
                   title TEXT NOT NULL CHECK(length(CAST(title AS BLOB)) BETWEEN 1 AND 240),
                   done INTEGER NOT NULL CHECK(done IN (0, 1))
               )"""
        )


def validate_input(value: Any) -> tuple[str, bool]:
    if not isinstance(value, dict) or set(value) != {"title", "done"}:
        abort(400, description="expected exactly title and done")
    title, done = value["title"], value["done"]
    if not isinstance(title, str) or type(done) is not bool:
        abort(400, description="title must be text and done must be boolean")
    try:
        size = len(title.encode("utf-8"))
    except UnicodeEncodeError:
        abort(400, description="title must be valid UTF-8")
    if not 1 <= size <= 240:
        abort(400, description="title must be between 1 and 240 UTF-8 bytes")
    return title, done


def require_task_id(task_id: int) -> None:
    if not 1 <= task_id <= MAX_I64:
        abort(400, description="id must be between 1 and i64::MAX")


def row_value(row: sqlite3.Row) -> dict[str, Any]:
    return {"id": row["id"], "title": row["title"], "done": bool(row["done"])}


def create_app(database: str | None = None) -> Quart:
    database = database or os.environ.get("TASKS_DB", "quart-tasks.sqlite")
    initialize(database)
    app = Quart(__name__)

    @app.get("/health")
    async def health():
        response = jsonify("ok")
        ready_token = os.environ.get("TASKS_TEST_READY_TOKEN")
        if ready_token:
            response.headers["X-Task-Ready"] = ready_token
        return response

    @app.get("/api/tasks")
    async def list_tasks():
        def load():
            with connect(database) as connection:
                return connection.execute(
                    "SELECT id, title, done FROM tasks ORDER BY id DESC LIMIT 100"
                ).fetchall()

        return jsonify([row_value(row) for row in await asyncio.to_thread(load)])

    @app.get("/api/tasks/<int:task_id>")
    async def get_task(task_id: int):
        require_task_id(task_id)

        def load():
            with connect(database) as connection:
                return connection.execute(
                    "SELECT id, title, done FROM tasks WHERE id = ?", (task_id,)
                ).fetchone()

        row = await asyncio.to_thread(load)
        if row is None:
            abort(404, description="task not found")
        return jsonify(row_value(row))

    @app.post("/api/tasks")
    async def create_task():
        title, done = validate_input(await request.get_json())

        def save():
            with connect(database) as connection:
                cursor = connection.execute(
                    "INSERT INTO tasks(title, done) VALUES (?, ?)", (title, int(done))
                )
                return cursor.lastrowid

        task_id = await asyncio.to_thread(save)
        return jsonify({"id": task_id, "title": title, "done": done})

    @app.put("/api/tasks/<int:task_id>")
    async def update_task(task_id: int):
        require_task_id(task_id)
        title, done = validate_input(await request.get_json())

        def save():
            with connect(database) as connection:
                return connection.execute(
                    "UPDATE tasks SET title = ?, done = ? WHERE id = ?",
                    (title, int(done), task_id),
                ).rowcount

        if await asyncio.to_thread(save) == 0:
            abort(404, description="task not found")
        return jsonify({"id": task_id, "title": title, "done": done})

    @app.delete("/api/tasks/<int:task_id>")
    async def delete_task(task_id: int):
        require_task_id(task_id)

        def remove():
            with connect(database) as connection:
                return connection.execute("DELETE FROM tasks WHERE id = ?", (task_id,)).rowcount

        return jsonify(await asyncio.to_thread(remove))

    @app.get("/api/stats")
    async def stats():
        def load():
            with connect(database) as connection:
                return connection.execute(
                    "SELECT COUNT(*) AS total, COALESCE(SUM(done), 0) AS completed FROM tasks"
                ).fetchone()

        row = await asyncio.to_thread(load)
        return jsonify({"total": row["total"], "completed": row["completed"]})

    return app


app = create_app()
