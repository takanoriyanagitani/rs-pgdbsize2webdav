use std::io;

use std::time::SystemTime;

use std::io::SeekFrom;

use futures_util::FutureExt;
use futures_util::Stream;
use futures_util::StreamExt;

use hyper::body::Buf;
use hyper::body::Bytes;

use sqlx::postgres::PgPool;

use dav_server::fakels;
use dav_server::ls::DavLockSystem;

use dav_server::fs::DavDirEntry;
use dav_server::fs::DavFile;
use dav_server::fs::DavFileSystem;
use dav_server::fs::DavMetaData;
use dav_server::fs::FsError;
use dav_server::fs::FsFuture;
use dav_server::fs::FsResult;
use dav_server::fs::FsStream;
use dav_server::fs::OpenOptions;
use dav_server::fs::ReadDirMeta;

use dav_server::davpath::DavPath;

/// Creates a fake lock system([`DavLockSystem`]).
pub fn fake_lock_system() -> impl DavLockSystem {
    fakels::FakeLs {}
}

/// Implements [`DavMetaData`].
#[derive(Debug, Clone)]
pub struct DbSizeFileMetadata {
    pub dbsize_string: String,
}

impl DavMetaData for DbSizeFileMetadata {
    fn len(&self) -> u64 {
        self.dbsize_string.len() as u64
    }

    fn modified(&self) -> FsResult<SystemTime> {
        Ok(SystemTime::now())
    }

    fn is_dir(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub struct FileMetadataEmpty {}

impl DavMetaData for FileMetadataEmpty {
    fn len(&self) -> u64 {
        0
    }

    fn modified(&self) -> FsResult<SystemTime> {
        Ok(SystemTime::now())
    }

    fn is_dir(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub struct DirMetadataEmpty {}

impl DavMetaData for DirMetadataEmpty {
    fn len(&self) -> u64 {
        0
    }

    fn modified(&self) -> FsResult<SystemTime> {
        Ok(SystemTime::now())
    }

    fn is_dir(&self) -> bool {
        true
    }
}

/// Implements [`DavFile`].
#[derive(Debug, Clone)]
pub struct DbInfoFile {
    pub name: String,
    pub dbsize_string: String,
}

impl DavFile for DbInfoFile {
    fn metadata(&mut self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        let sz_str: String = self.dbsize_string.clone();
        async move {
            Ok(Box::new(DbSizeFileMetadata {
                dbsize_string: sz_str,
            }) as Box<dyn DavMetaData>)
        }
        .boxed()
    }

    fn write_buf(&mut self, _buf: Box<dyn Buf + Send>) -> FsFuture<'_, ()> {
        async move { Err(FsError::Forbidden) }.boxed()
    }

    fn write_bytes(&mut self, _buf: Bytes) -> FsFuture<'_, ()> {
        async move { Err(FsError::Forbidden) }.boxed()
    }

    fn read_bytes(&mut self, count: usize) -> FsFuture<'_, Bytes> {
        async move {
            let sz_str: &str = &self.dbsize_string;
            let original: &[u8] = sz_str.as_bytes();
            let i = original.iter();
            let taken = i.take(count);
            let sz_bytes: Vec<u8> = taken.copied().collect();
            let b: Bytes = Bytes::from(sz_bytes);
            Ok(b)
        }
        .boxed()
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsFuture<'_, u64> {
        let name: &str = &self.name;
        eprintln!("[DbInfoFile] no seek for now: {name}");
        async move { Err(FsError::NotImplemented) }.boxed()
    }

    fn flush(&mut self) -> FsFuture<'_, ()> {
        let name: &str = &self.name;
        eprintln!("[DbInfoFile] no flush for now: {name}");
        async move { Err(FsError::NotImplemented) }.boxed()
    }
}

/// Implementation of [`DavFile`] for unknown db.
#[derive(Debug, Clone)]
pub struct UnknownNameFile {
    pub name: String,
}

impl DavFile for UnknownNameFile {
    fn metadata(&mut self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        let name: &str = &self.name;
        eprintln!("[UnknownNameFile::metadata]: {name}");
        async move { Ok(Box::new(FileMetadataEmpty {}) as Box<dyn DavMetaData>) }.boxed()
    }

    fn write_buf(&mut self, _buf: Box<dyn Buf + Send>) -> FsFuture<'_, ()> {
        async move { Err(FsError::Forbidden) }.boxed()
    }

    fn write_bytes(&mut self, _buf: Bytes) -> FsFuture<'_, ()> {
        async move { Err(FsError::Forbidden) }.boxed()
    }

    fn read_bytes(&mut self, _count: usize) -> FsFuture<'_, Bytes> {
        let name: &str = &self.name;
        eprintln!("[UnknownNameFile] no read_bytes for unknown: {name}");
        async move { Err(FsError::NotImplemented) }.boxed()
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsFuture<'_, u64> {
        let name: &str = &self.name;
        eprintln!("[UnknownNameFile] no seek for unknown: {name}");
        async move { Err(FsError::NotImplemented) }.boxed()
    }

    fn flush(&mut self) -> FsFuture<'_, ()> {
        let name: &str = &self.name;
        eprintln!("[UnknownNameFile] no flush for unknown: {name}");
        async move { Err(FsError::NotImplemented) }.boxed()
    }
}

/// Implements [`DavDirEntry`].
pub struct DirentDbName {
    pub name: String,
    pub dbsize_string: String,
}

impl DavDirEntry for DirentDbName {
    fn name(&self) -> Vec<u8> {
        self.name.as_bytes().into()
    }

    fn metadata(&self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        async move {
            Ok(Box::new(DbSizeFileMetadata {
                dbsize_string: self.dbsize_string.clone(),
            }) as Box<dyn DavMetaData>)
        }
        .boxed()
    }
}

/// Implements [`DavFileSystem`].
#[derive(Debug, Clone)]
pub struct DbSizeFs<S, L> {
    pub db_size_source: S,
    pub db_list_source: L,
}

/// Gets the list of db names using the [`PgPool`].
pub async fn get_db_names(p: &PgPool) -> Result<Vec<String>, io::Error> {
    sqlx::query_scalar!(
        r#"(
            SELECT datname
            FROM pg_database
            ORDER BY datname
        )"#
    )
    .fetch_all(p)
    .await
    .map_err(io::Error::other)
}

/// Gets the list of db names using the [`PgPool`].
pub async fn get_db_name_stream(p: &PgPool) -> impl Stream<Item = Result<String, io::Error>> {
    sqlx::query_scalar!(
        r#"(
            SELECT datname
            FROM pg_database
            ORDER BY datname
        )"#
    )
    .fetch(p)
    .map(|rslt| rslt.map_err(io::Error::other))
}

/// Gets the size of the specified db using the [`PgPool`].
pub async fn get_dbsize_by_dbname(p: &PgPool, dbname: &str) -> Result<i64, io::Error> {
    let dsz: Option<i64> = sqlx::query_scalar!(
        r#"(
            SELECT PG_DATABASE_SIZE(datname)::BIGINT AS dbsize
            FROM pg_database
            WHERE datname=$1::TEXT
        )"#,
        dbname
    )
    .fetch_one(p)
    .await
    .map_err(io::Error::other)?;
    Ok(dsz.unwrap_or(0))
}

/// Implements [`PgDbnameSource`] and [`PgDbSizeSource`].
#[derive(Debug, Clone)]
pub struct PostgresqlPool {
    pub p: PgPool,
}

/// Gets list of db names.
#[async_trait::async_trait]
pub trait PgDbnameSource {
    async fn get_db_names(&self) -> Result<Vec<String>, io::Error>;
}

/// Gets the size of the db.
#[async_trait::async_trait]
pub trait PgDbSizeSource {
    async fn get_db_size_by_name(&self, name: &str) -> Result<i64, io::Error>;
}

#[async_trait::async_trait]
impl PgDbnameSource for PostgresqlPool {
    async fn get_db_names(&self) -> Result<Vec<String>, io::Error> {
        get_db_names(&self.p).await
    }
}

#[async_trait::async_trait]
impl PgDbSizeSource for PostgresqlPool {
    async fn get_db_size_by_name(&self, name: &str) -> Result<i64, io::Error> {
        get_dbsize_by_dbname(&self.p, name).await
    }
}

impl<S, L> DbSizeFs<S, L>
where
    S: PgDbSizeSource,
    L: PgDbnameSource,
{
    pub async fn get_names(&self) -> Result<Vec<String>, io::Error> {
        self.db_list_source.get_db_names().await
    }

    pub async fn get_size_by_name(&self, dbname: &str) -> Result<i64, io::Error> {
        self.db_size_source.get_db_size_by_name(dbname).await
    }
}

impl<S, L> DavFileSystem for DbSizeFs<S, L>
where
    S: PgDbSizeSource + Sync,
    L: PgDbnameSource + Sync,
{
    fn metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, Box<dyn DavMetaData>> {
        let basename: &str = path.file_name().unwrap_or_default();
        eprintln!("[metadata] path: {basename}");
        if basename.is_empty() {
            return async move { Ok(Box::new(DirMetadataEmpty {}) as Box<dyn DavMetaData>) }
                .boxed();
        }
        async move { Ok(Box::new(FileMetadataEmpty {}) as Box<dyn DavMetaData>) }.boxed()
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        _meta: ReadDirMeta,
    ) -> FsFuture<'a, FsStream<Box<dyn DavDirEntry>>> {
        async move {
            let pat: &str = path.file_name().unwrap_or_default();
            if pat.is_empty() {
                let names: Vec<String> = self.get_names().await?;
                let strm = futures_util::stream::iter(names)
                    .then(|name: String| async move {
                        let rslt: Result<i64, _> = self.get_size_by_name(&name).await;
                        let rstr: Result<String, _> = rslt.map(|i| format!("{i}"));
                        match rstr {
                            Ok(sz) => DirentDbName {
                                name,
                                dbsize_string: sz,
                            },
                            Err(e) => {
                                let dbsize_string: String =
                                    format!("NO SIZE INFO AVAILABLE FOR {name}: {e}");
                                DirentDbName {
                                    name,
                                    dbsize_string,
                                }
                            }
                        }
                    })
                    .map(Box::new)
                    .map(|b| b as Box<dyn DavDirEntry>);
                let collected: Vec<Box<dyn DavDirEntry>> = strm.collect().await;
                let strm = futures_util::stream::iter(collected).map(Ok);
                return Ok(Box::pin(strm) as FsStream<Box<dyn DavDirEntry>>);
            }

            eprintln!("[read_dir] path: {path}");
            let strm = futures_util::stream::iter(vec![]).map(Ok);
            Ok(Box::pin(strm) as FsStream<Box<dyn DavDirEntry>>)
        }
        .boxed()
    }

    fn open<'a>(
        &'a self,
        path: &'a DavPath,
        _options: OpenOptions,
    ) -> FsFuture<'a, Box<dyn DavFile>> {
        async move {
            let basename: &str = path.file_name().unwrap_or_default();
            let rsz: Result<i64, _> = self.get_size_by_name(basename).await;
            match rsz {
                Err(_) => Ok(Box::new(UnknownNameFile {
                    name: basename.into(),
                }) as Box<dyn DavFile>),
                Ok(sz) => Ok(Box::new(DbInfoFile {
                    name: basename.into(),
                    dbsize_string: format!("{sz}"),
                }) as Box<dyn DavFile>),
            }
        }
        .boxed()
    }
}

/// Returns [`DavFileSystem`] using the [`PgPool`].
pub fn pool2dbsize_fs(p: PgPool) -> impl DavFileSystem + Clone {
    let ps: PgPool = p.clone();
    let pl: PgPool = p;

    DbSizeFs {
        db_size_source: PostgresqlPool { p: ps },
        db_list_source: PostgresqlPool { p: pl },
    }
}
