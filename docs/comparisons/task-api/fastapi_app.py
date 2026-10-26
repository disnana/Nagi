"""FastAPI implementation used by the Nagi task API comparison."""

import os
import sqlite3
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

from fastapi import FastAPI, HTTPException, Request, Response
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from pydantic import BaseModel, ConfigDict, StrictBool, StrictStr, field_validator


MAX_I64 = 9_223_372_036_854_775_807


class Task(BaseModel):
    id: int
    title: str
    done: bool


class SaveTask(BaseModel):
    model_config = ConfigDict(extra="forbid")

    title: StrictStr
    done: StrictBool

    @field_validator("title")
    @classmethod
    def title_is_1_to_240_utf8_bytes(cls, value: str) -> str:
        try:
            size = len(value.encode("utf-8"))
        except UnicodeEncodeError:
            raise ValueError("title must be valid UTF-8") from None
        if not 1 <= size <= 240:
            raise ValueError("title must be between 1 and 240 UTF-8 bytes")
        return value


@contextmanager
def connect(database: str) -> Iterator[sqlite3.Connection]:
    connection = sqlite3.connect(database)
    connection.row_factory = sqlite3.Row
    try:
        with connection:
            yield connection
    finally:
        connection.close()


def require_task_id(task_id: int) -> None:
    if not 1 <= task_id <= MAX_I64:
        raise HTTPException(400, "id must be between 1 and i64::MAX")


def create_app(database: str | None = None) -> FastAPI:
    database = database or os.environ.get("TASKS_DB", "fastapi-tasks.sqlite")
    Path(database).parent.mkdir(parents=True, exist_ok=True)
    with connect(database) as connection:
        connection.execute(
            """CREATE TABLE IF NOT EXISTS tasks(
                   id INTEGER PRIMARY KEY,
                   title TEXT NOT NULL CHECK(length(CAST(title AS BLOB)) BETWEEN 1 AND 240),
                   done INTEGER NOT NULL CHECK(done IN (0, 1))
               )"""
        )

    app = FastAPI(title="Task JSON API")

    @app.exception_handler(RequestValidationError)
    async def invalid_request(
        _request: Request, _problem: RequestValidationError
    ) -> JSONResponse:
        # Do not reflect invalid Unicode from the request in an error response.
        return JSONResponse(
            status_code=422,
            content={
                "detail": [
                    {"loc": ["body"], "msg": "invalid request", "type": "value_error"}
                ]
            },
        )

    @app.get("/health")
    def health(response: Response) -> str:
        ready_token = os.environ.get("TASKS_TEST_READY_TOKEN")
        if ready_token:
            response.headers["X-Task-Ready"] = ready_token
        return "ok"

    @app.get("/api/tasks", response_model=list[Task])
    def list_tasks() -> list[Task]:
        with connect(database) as connection:
            rows = connection.execute(
                "SELECT id, title, done FROM tasks ORDER BY id DESC LIMIT 100"
            ).fetchall()
        return [Task(id=row["id"], title=row["title"], done=bool(row["done"])) for row in rows]

    @app.get("/api/tasks/{task_id}", response_model=Task)
    def get_task(task_id: int) -> Task:
        require_task_id(task_id)
        with connect(database) as connection:
            row = connection.execute(
                "SELECT id, title, done FROM tasks WHERE id = ?", (task_id,)
            ).fetchone()
        if row is None:
            raise HTTPException(404, "task not found")
        return Task(id=row["id"], title=row["title"], done=bool(row["done"]))

    @app.post("/api/tasks", response_model=Task)
    def create_task(value: SaveTask) -> Task:
        with connect(database) as connection:
            cursor = connection.execute(
                "INSERT INTO tasks(title, done) VALUES (?, ?)", (value.title, int(value.done))
            )
            task_id = cursor.lastrowid
        return Task(id=task_id, title=value.title, done=value.done)

    @app.put("/api/tasks/{task_id}", response_model=Task)
    def update_task(task_id: int, value: SaveTask) -> Task:
        require_task_id(task_id)
        with connect(database) as connection:
            cursor = connection.execute(
                "UPDATE tasks SET title = ?, done = ? WHERE id = ?",
                (value.title, int(value.done), task_id),
            )
        if cursor.rowcount == 0:
            raise HTTPException(404, "task not found")
        return Task(id=task_id, title=value.title, done=value.done)

    @app.delete("/api/tasks/{task_id}")
    def delete_task(task_id: int) -> int:
        require_task_id(task_id)
        with connect(database) as connection:
            cursor = connection.execute("DELETE FROM tasks WHERE id = ?", (task_id,))
        return cursor.rowcount

    @app.get("/api/stats")
    def stats() -> dict[str, int]:
        with connect(database) as connection:
            row = connection.execute(
                "SELECT COUNT(*) AS total, COALESCE(SUM(done), 0) AS completed FROM tasks"
            ).fetchone()
        return {"total": row["total"], "completed": row["completed"]}

    return app


app = create_app()
