use sqlx::pool::Pool;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Sqlite;

pub async fn create_sqlite_pool() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        // TODO db path in conf
        .connect("sqlite/eloran.db")
        .await
        .unwrap();
    pool
}

pub async fn create_sqlite_connection() -> sqlx::pool::PoolConnection<sqlx::Sqlite> {
    let pool = create_sqlite_pool().await;
    let conn = pool.acquire().await.unwrap();
    conn
}
