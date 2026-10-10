use rusqlite::Row;

pub trait FromRow: Send + 'static {
    fn columns() -> &'static [&'static str];
    fn read(row: &Row<'_>, indices: &[usize]) -> rusqlite::Result<Self>
    where
        Self: Sized;
}
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
