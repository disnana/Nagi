use crate::Error;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::sync::{mpsc, oneshot};

pub trait FromRow: Send + 'static {
    fn columns() -> &'static [&'static str];
    fn read(row: &Row<'_>, indices: &[usize]) -> rusqlite::Result<Self>
    where
        Self: Sized;
}
pub enum Sql {
    Static(&'static str),
    Owned(String),
}
impl From<&'static str> for Sql {
    fn from(s: &'static str) -> Self {
        Self::Static(s)
    }
}
impl Sql {
    fn as_str(&self) -> &str {
        match self {
            Self::Static(s) => s,
            Self::Owned(s) => s,
        }
    }
}
type Job = Box<dyn FnOnce(&mut Connection) + Send + 'static>;
static LIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);
struct Inner {
    tx: Option<mpsc::Sender<Job>>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(t) = self.thread.lock().unwrap().take() {
            let _ = t.join();
        }
    }
}
#[derive(Clone)]
pub struct Db {
    inner: Arc<Inner>,
}
impl Db {
    pub fn live_workers() -> usize {
        LIVE_WORKERS.load(Ordering::SeqCst)
    }
    pub async fn open(path: &str) -> Result<Self, Error> {
        let path = path.to_owned();
        let (tx, mut rx) = mpsc::channel::<Job>(64);
        let (ready_tx, ready_rx) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("nagi-sqlite".into())
            .spawn(move || {
                let mut conn = match Connection::open(path) {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = ready_tx.send(Err(Error::from(e)));
                        return;
                    }
                };
                conn.busy_timeout(std::time::Duration::from_millis(500))
                    .ok();
                conn.set_prepared_statement_cache_capacity(32);
                LIVE_WORKERS.fetch_add(1, Ordering::SeqCst);
                struct Live;
                impl Drop for Live {
                    fn drop(&mut self) {
                        LIVE_WORKERS.fetch_sub(1, Ordering::SeqCst);
                    }
                }
                let _live = Live;
                if ready_tx.send(Ok(())).is_err() {
                    return;
                }
                while let Some(job) = rx.blocking_recv() {
                    job(&mut conn);
                }
            })
            .map_err(|e| Error::internal(e.to_string()))?;
        // receiverがready待ち中にキャンセルされてもsenderを落としてworkerを終了させる。
        ready_rx
            .await
            .map_err(|_| Error::internal("DB worker initialization failed"))??;
        Ok(Self {
            inner: Arc::new(Inner {
                tx: Some(tx),
                thread: Mutex::new(Some(thread)),
            }),
        })
    }
    async fn call<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        let (tx, rx) = oneshot::channel();
        let job: Job = Box::new(move |c| {
            let _ = tx.send(f(c));
        });
        self.inner
            .tx
            .as_ref()
            .unwrap()
            .send(job)
            .await
            .map_err(|_| Error::internal("DB worker stopped"))?;
        rx.await.map_err(|_| Error::internal("DB reply lost"))?
    }
    pub async fn exec(&self, sql: impl Into<Sql>) -> Result<i64, Error> {
        let sql = sql.into();
        self.call(move |c| {
            let before = c.total_changes();
            c.execute_batch(sql.as_str())?;
            i64::try_from(c.total_changes() - before)
                .map_err(|_| Error::internal("affected row count exceeds i64"))
        })
        .await
    }
    pub async fn query<T: FromRow>(
        &self,
        sql: impl Into<Sql>,
        id: i64,
    ) -> Result<Option<T>, Error> {
        let sql = sql.into();
        self.call(move |c| {
            let mut stmt = c.prepare_cached(sql.as_str())?;
            let ix: TIndices = indices::<T>(&stmt)?;
            Ok(stmt.query_row([id], |row| T::read(row, &ix)).optional()?)
        })
        .await
    }
    pub async fn all<T: FromRow>(&self, sql: impl Into<Sql>) -> Result<Vec<T>, Error> {
        let sql = sql.into();
        self.call(move |c| {
            let mut stmt = c.prepare_cached(sql.as_str())?;
            let ix = indices::<T>(&stmt)?;
            let iter = stmt.query_map([], |row| T::read(row, &ix))?;
            let mut out = vec![];
            for row in iter {
                out.push(row?);
            }
            Ok(out)
        })
        .await
    }
    pub async fn insert<T: FromRow>(
        &self,
        sql: impl Into<Sql>,
        name: String,
        age: i32,
    ) -> Result<T, Error> {
        let sql = sql.into();
        self.call(move |c| {
            let mut stmt = c.prepare_cached(sql.as_str())?;
            let ix = indices::<T>(&stmt)?;
            Ok(stmt.query_row(params![name, age], |row| T::read(row, &ix))?)
        })
        .await
    }
    pub async fn update<T: FromRow>(
        &self,
        sql: impl Into<Sql>,
        id: i64,
        name: String,
        age: i32,
    ) -> Result<T, Error> {
        let sql = sql.into();
        self.call(move |c| {
            let mut stmt = c.prepare_cached(sql.as_str())?;
            let ix = indices::<T>(&stmt)?;
            stmt.query_row(params![id, name, age], |row| T::read(row, &ix))
                .optional()?
                .ok_or_else(Error::not_found)
        })
        .await
    }
    pub async fn write(&self, sql: impl Into<Sql>, id: i64) -> Result<i64, Error> {
        let sql = sql.into();
        self.call(move |c| Ok(c.prepare_cached(sql.as_str())?.execute([id])? as i64))
            .await
    }
}
type TIndices = ColumnIndices;
pub struct ColumnIndices {
    inline: [usize; 16],
    len: usize,
    overflow: Vec<usize>,
}
impl std::ops::Deref for ColumnIndices {
    type Target = [usize];
    fn deref(&self) -> &[usize] {
        if self.len <= 16 {
            &self.inline[..self.len]
        } else {
            &self.overflow
        }
    }
}
pub fn indices<T: FromRow>(s: &rusqlite::Statement<'_>) -> rusqlite::Result<ColumnIndices> {
    // 通常の小さいclassではindices用のheap allocationを作らない。
    let columns = T::columns();
    let mut ix = ColumnIndices {
        inline: [0; 16],
        len: columns.len(),
        overflow: Vec::new(),
    };
    if columns.len() <= 16 {
        for (i, n) in columns.iter().enumerate() {
            ix.inline[i] = s.column_index(n)?;
        }
    } else {
        for n in columns {
            ix.overflow.push(s.column_index(n)?);
        }
    }
    Ok(ix)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug)]
    struct User {
        id: i64,
        name: String,
        age: i32,
    }
    impl FromRow for User {
        fn columns() -> &'static [&'static str] {
            &["id", "name", "age"]
        }
        fn read(r: &Row<'_>, ix: &[usize]) -> rusqlite::Result<Self> {
            Ok(Self {
                id: r.get(ix[0])?,
                name: r.get(ix[1])?,
                age: r.get(ix[2])?,
            })
        }
    }
    #[tokio::test]
    async fn exec_counts_only_changes_in_the_current_batch() {
        let d = Db::open(":memory:").await.unwrap();
        assert_eq!(d.exec("CREATE TABLE items(id INTEGER)").await.unwrap(), 0);
        assert_eq!(
            d.exec("INSERT INTO items VALUES (1), (2), (3)")
                .await
                .unwrap(),
            3
        );
        assert_eq!(d.exec("CREATE TABLE other(id INTEGER)").await.unwrap(), 0);
        assert_eq!(d.exec("SELECT * FROM items").await.unwrap(), 0);
        assert_eq!(
            d.exec("UPDATE items SET id = 4 WHERE id = 999")
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            d.exec("UPDATE items SET id = id + 1; DELETE FROM items WHERE id = 2")
                .await
                .unwrap(),
            4
        );
        assert_eq!(d.exec("SELECT * FROM items").await.unwrap(), 0);
    }
    #[tokio::test]
    async fn exec_counts_trigger_changes_and_preserves_sql_errors() {
        let d = Db::open(":memory:").await.unwrap();
        d.exec("CREATE TABLE items(id INTEGER); CREATE TABLE audit(id INTEGER); CREATE TRIGGER log_insert AFTER INSERT ON items BEGIN INSERT INTO audit VALUES (NEW.id); END;").await.unwrap();
        assert_eq!(d.exec("INSERT INTO items VALUES (1)").await.unwrap(), 2);
        assert!(d.exec("INSERT INTO missing VALUES (1)").await.is_err());
        assert_eq!(d.exec("SELECT * FROM audit").await.unwrap(), 0);
    }
    #[tokio::test]
    async fn crud_and_parameterization() {
        let d = Db::open(":memory:").await.unwrap();
        d.exec("CREATE TABLE users(id INTEGER PRIMARY KEY,name TEXT,age INTEGER)")
            .await
            .unwrap();
        let u: User = d
            .insert(
                "INSERT INTO users(name,age)VALUES(?1,?2)RETURNING id,name,age",
                "x'); DROP TABLE users;--".into(),
                18,
            )
            .await
            .unwrap();
        assert_eq!(u.id, 1);
        let u = d
            .query::<User>("SELECT id,name,age FROM users WHERE id=?1", 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(u.age, 18);
        assert!(u.name.contains("DROP"));
        assert!(d
            .query::<User>("SELECT id,name,age FROM users WHERE id=?1", 999)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            d.write("DELETE FROM users WHERE id=?1", 1).await.unwrap(),
            1
        );
    }
    #[tokio::test]
    async fn missing_column_is_error() {
        let d = Db::open(":memory:").await.unwrap();
        assert!(d.query::<User>("SELECT ?1 AS id", 1).await.is_err());
    }
    #[tokio::test]
    async fn runtime_type_mismatch() {
        let d = Db::open(":memory:").await.unwrap();
        assert!(d
            .query::<User>("SELECT ?1 AS id, 'tp' AS name, 'bad' AS age", 1)
            .await
            .is_err());
    }
    #[tokio::test]
    async fn column_order_independent() {
        let d = Db::open(":memory:").await.unwrap();
        let u = d
            .query::<User>("SELECT 18 AS age, 'tp' AS name, ?1 AS id", 2)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(u.id, 2);
    }
}
