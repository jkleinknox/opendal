use crate::raw::adapters::kv;
use crate::raw::input::{BlockingReader, Reader};
use crate::services::sled::dir_pager::DirPager;
use crate::*;
use crate::{
    raw::*, Builder, Error, ErrorKind, OpCreate, OpDelete, OpList, OpRead, OpStat, OpWrite, Scheme,
};
use anyhow::Context;
use async_trait::async_trait;
use futures::AsyncReadExt;
use sled::IVec;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::path::Path;

/// Sled service support.
///
/// # Capabilities
///
/// This service can be used to:
///
/// - [x] read
/// - [x] write
/// - [ ] ~~list~~
/// - [ ] ~~presign~~
/// - [ ] ~~multipart~~
/// - [x] blocking
///
/// # Note
///
/// The storage format for this service is not **stable** yet.
///
/// PLEASE DON'T USE THIS SERVICE FOR PERSIST DATA.
///
/// # Configuration
///
/// - `datadir`: Set the path to the rocksdb data directory
///
/// You can refer to [`SledBuilder`]'s docs for more information
///
/// # Example
///
/// ## Via Builder
///
/// ```no_run
/// use anyhow::Result;
/// use opendal::services::Sled;
/// use opendal::Object;
/// use opendal::Operator;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut builder = Sled::default();
///     builder.datadir("/tmp/opendal/sled");
///
///     let op: Operator = Operator::create(builder)?.finish();
///     let _: Object = op.object("test_file");
///     Ok(())
/// }
/// ```
#[derive(Default)]
pub struct SledBuilder {
    /// That path to the sled data directory.
    datadir: Option<String>,
}

impl SledBuilder {
    /// Set the path to the sled data directory. Will create if not exists.
    pub fn datadir(&mut self, path: &str) -> &mut Self {
        self.datadir = Some(path.into());
        self
    }
}

impl Builder for SledBuilder {
    const SCHEME: Scheme = Scheme::Sled;
    type Accessor = SledBackend;

    fn from_map(map: HashMap<String, String>) -> Self {
        let mut builder = SledBuilder::default();

        map.get("datadir").map(|v| builder.datadir(v));

        builder
    }

    fn build(&mut self) -> Result<Self::Accessor> {
        let datadir_path = self.datadir.take().ok_or_else(|| {
            Error::new(
                ErrorKind::BackendConfigInvalid,
                "datadir is required but not set",
            )
            .with_context("service", Scheme::Sled)
        })?;

        let db = sled::open(&datadir_path).map_err(|e| {
            Error::new(ErrorKind::BackendConfigInvalid, "open db")
                .with_context("service", Scheme::Sled)
                .with_context("datadir", datadir_path.clone())
                .set_source(e)
        })?;

        Ok(SledBackend::new(Adapter {
            datadir: datadir_path,
            db,
        }))
    }
}

/// Backend for sled services.
pub type SledBackend = kv::Backend<Adapter>;

#[derive(Clone)]
pub struct Adapter {
    datadir: String,
    db: sled::Db,
}

impl Debug for Adapter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut ds = f.debug_struct("Adapter");
        ds.field("path", &self.datadir);
        ds.finish()
    }
}

#[async_trait]
impl kv::Adapter for Adapter {
    fn metadata(&self) -> kv::Metadata {
        kv::Metadata::new(
            Scheme::Sled,
            &self.datadir,
            AccessorCapability::Read | AccessorCapability::Write | AccessorCapability::Blocking,
        )
    }

    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>> {
        self.blocking_get(path)
    }

    fn blocking_get(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let parent = get_parent(path);
        let basename = get_basename(path);

        let tree = self.db.open_tree(parent).map_err(|e| {
            Error::new(ErrorKind::Unexpected, "Unable to open tree")
                .with_context("input", path)
                .set_source(e)
        })?;

        Ok(tree.get(basename).map_err(|e| {
            Error::new(ErrorKind::Unexpected, "Unable to get")
                .with_context("input", basename)
                .set_source(e)
        })?.map(|v| v.to_vec()))
    }

    async fn set(&self, path: &str, value: &[u8]) -> Result<()> {
        self.blocking_set(path, value)
    }

    fn blocking_set(&self, path: &str, value: &[u8]) -> Result<()> {
        let parent = get_parent(path);
        let basename = get_basename(path);

        let tree = self.db.open_tree(parent).map_err(|e| {
            Error::new(ErrorKind::Unexpected, "Unable to open tree")
                .with_context("input", path)
                .set_source(e)
        })?;

        tree.insert(basename, value).map_err(|e| {
            Error::new(ErrorKind::Unexpected, "Unable to set")
                .with_context("input", basename)
                .set_source(e)
        })?;

        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<()> {
        self.blocking_delete(path)
    }

    fn blocking_delete(&self, path: &str) -> Result<()> {
        let parent = get_parent(path);
        let basename = get_basename(path);

        let tree = self.db.open_tree(parent).map_err(|e| {
            Error::new(ErrorKind::Unexpected, "Unable to open tree")
                .with_context("input", path)
                .set_source(e)
        })?;

        tree.remove(basename).map_err(|e| {
            Error::new(ErrorKind::Unexpected, "Unable to delete")
                .with_context("input", basename)
                .set_source(e)
        })?;

        Ok(())
    }
}

// #[derive(Debug)]
// pub struct SledBackend {
//     datadir: String,
//     root: String,
//     db: sled::Db,
// }
//
// impl SledBackend {
//     fn split_path(&self, path: &str) -> (String, String) {
//         let absolute_path = build_rooted_abs_path(&self.root, path);
//         // let p = Path::new(&absolute_path);
//         // let path_str = p.to_str().unwrap();
//
//         // let parent = p.parent().unwrap_or(Path::new("/")).to_str().unwrap();
//         // let file_name = p.file_name().unwrap().to_str().unwrap();
//
//
//         (get_parent(&absolute_path).to_string(), get_basename(&absolute_path).to_string())
//     }
// }
//
//
// #[async_trait]
// impl Accessor for SledBackend {
//     type Reader = output::Cursor;
//     type BlockingReader = output::Cursor;
//
//     fn metadata(&self) -> AccessorMetadata {
//         let mut am = AccessorMetadata::default();
//         am.set_scheme(Scheme::Sled).set_capabilities(
//             AccessorCapability::Read
//                 | AccessorCapability::Write
//                 | AccessorCapability::List
//                 | AccessorCapability::Blocking,
//         );
//
//         am
//     }
//
//     async fn create(&self, path: &str, args: OpCreate) -> Result<RpCreate> {
//         self.blocking_create(path, args)
//     }
//
//     async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
//         self.blocking_read(path, args)
//     }
//
//     async fn write(&self, path: &str, args: OpWrite, mut r: Reader) -> Result<RpWrite> {
//         let (parent, file_name) = self.split_path(path);
//
//         let tree = self.db.open_tree(parent).unwrap();
//
//         let mut read_vec = Vec::with_capacity(args.size() as usize);
//         let bytes_read = r.read_to_end(&mut read_vec).await.unwrap();
//
//         tree.insert(file_name, read_vec).unwrap();
//
//         Ok(RpWrite::new(bytes_read as u64))
//     }
//
//     async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
//         self.blocking_stat(path, args)
//     }
//
//     async fn delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
//         self.blocking_delete(path, args)
//     }
//
//     async fn list(&self, path: &str, args: OpList) -> Result<(RpList, ObjectPager)> {
//         let absolute_path = build_rooted_abs_path(&self.root, path);
//
//         let tree = self.db.open_tree(absolute_path).unwrap();
//
//         Ok((RpList::default(), Box::new(DirPager::new(tree, 256))))
//     }
//
//     fn blocking_create(&self, path: &str, args: OpCreate) -> Result<RpCreate> {
//         if args.mode() == ObjectMode::FILE {
//             let (parent, file_name) = self.split_path(path);
//
//             let tree = self.db.open_tree(parent).unwrap();
//             tree.insert(file_name, vec![]).unwrap();
//         }
//
//         Ok(RpCreate::default())
//     }
//
//     fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
//         let (parent, file_name) = self.split_path(path);
//
//         let tree = self.db.open_tree(parent).unwrap();
//
//         let subset = apply_range(tree.get(file_name).unwrap().unwrap().to_vec(), args.range());
//         let subset_len = subset.len() as u64;
//
//         let cursor = output::Cursor::from(subset);
//
//         Ok((RpRead::new(subset_len), cursor))
//     }
//
//     fn blocking_write(&self, path: &str, args: OpWrite, mut r: BlockingReader) -> Result<RpWrite> {
//         println!("write: {path}");
//         let (parent, file_name) = self.split_path(path);
//
//         println!("write: {parent}, {file_name}");
//
//         let tree = self.db.open_tree(parent).unwrap();
//
//         println!("write: {}", args.size());
//
//         let mut read_vec = Vec::with_capacity(args.size() as usize);
//         let bytes_read = r.read_to_end(&mut read_vec).unwrap();
//
//         println!("write: {}", read_vec.len());
//
//         tree.insert(file_name, read_vec).unwrap();
//
//         Ok(RpWrite::new(bytes_read as u64))
//     }
//
//     fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
//         let rel_path = build_rel_path(&self.root, path);
//
//         let absolute_path = build_rooted_abs_path(&self.root, &rel_path);
//
//         println!("blocking_stat: {}", path);
//         println!("blocking_stat: {}", rel_path);
//         println!("blocking_stat: {}", absolute_path);
//         // println!("blocking_stat: {path}");
//         //
//         // let (parent, basename) = self.split_path(path);
//         //
//         // println!("blocking_stat: {parent} {file_name}");
//         //
//         // let absolute_path = build_rooted_abs_path(&self.root, path);
//         // println!("blockign_stat: {}, {}", get_parent(&absolute_path), get_basename(&absolute_path));
//
//         let parent = get_parent(&absolute_path);
//         let basename = get_basename(&absolute_path);
//
//         println!("blocking_stat: {parent}, {basename}");
//
//         let tree = self.db.open_tree(parent).unwrap();
//         let size = tree.get(basename).unwrap().unwrap().len();
//         // assert!(tree.contains_key(basename).unwrap());
//
//         println!("blocking_stat: {}", tree.contains_key(basename).unwrap());
//         println!("blocking_stat: {size}");
//
//         Ok(RpStat::new(ObjectMetadata::new(ObjectMode::FILE).with_content_length(size as u64)))
//     }
//
//     fn blocking_delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
//         let (parent, file_name) = self.split_path(path);
//
//         let tree = self.db.open_tree(parent).unwrap();
//         tree.remove(file_name).unwrap().unwrap();
//
//         Ok(RpDelete::default())
//     }
//
//     fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, BlockingObjectPager)> {
//         println!("blocking_list::path: {path}");
//
//         let mut absolute_path = build_rooted_abs_path(&self.root, path);
//
//         // if absolute_path.ends_with('/') {
//         //     let _ = absolute_path.split_off(absolute_path.len() - 1);
//         // }
//
//         println!("blocking_list::absolute: {absolute_path}");
//
//         let tree = self.db.open_tree(absolute_path).unwrap();
//
//         Ok((RpList::default(), Box::new(DirPager::new(tree, 256))))
//     }
// }
//
// fn apply_range(mut bs: Vec<u8>, br: BytesRange) -> Vec<u8> {
//     match (br.offset(), br.size()) {
//         (Some(offset), Some(size)) => {
//             let mut bs = bs.split_off(offset as usize);
//             if (size as usize) < bs.len() {
//                 let _ = bs.split_off(size as usize);
//             }
//             bs
//         }
//         (Some(offset), None) => bs.split_off(offset as usize),
//         (None, Some(size)) => bs.split_off(bs.len() - size as usize),
//         (None, None) => bs,
//     }
// }
