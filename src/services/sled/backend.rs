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
use std::path::Path;

#[derive(Default)]
pub struct SledBuilder {
    datadir: Option<String>,
    root: Option<String>,
}

impl SledBuilder {
    pub fn datadir(&mut self, path: &str) -> &mut Self {
        self.datadir = Some(path.into());
        self
    }

    pub fn root(&mut self, root: &str) -> &mut Self {
        self.root = Some(root.into());
        self
    }
}

impl Builder for SledBuilder {
    const SCHEME: Scheme = Scheme::Sled;
    type Accessor = SledBackend;

    fn from_map(map: HashMap<String, String>) -> Self {
        let mut builder = SledBuilder::default();

        map.get("root").map(|v| builder.root(v));

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
                .with_context("root", datadir_path.clone())
                .set_source(e)
        })?;

        Ok(SledBackend {
            datadir: datadir_path,
            root: self.root.take().unwrap_or("/".to_string()),
            db,
        })
    }
}

#[derive(Debug)]
pub struct SledBackend {
    datadir: String,
    root: String,
    db: sled::Db,
}

impl SledBackend {
    fn split_path(&self, path: &str) -> (String, String) {
        let absolute_path = build_rooted_abs_path(&self.root, path);
        let p = Path::new(&absolute_path);

        let parent = p.parent().unwrap_or(Path::new("/")).to_str().unwrap();
        let file_name = p.file_name().unwrap().to_str().unwrap();

        (parent.to_string(), file_name.to_string())
    }
}

#[async_trait]
impl Accessor for SledBackend {
    type Reader = output::Cursor;
    type BlockingReader = output::Cursor;

    fn metadata(&self) -> AccessorMetadata {
        let mut am = AccessorMetadata::default();
        am.set_scheme(Scheme::Sled).set_capabilities(
            AccessorCapability::Read
                | AccessorCapability::Write
                | AccessorCapability::List
                | AccessorCapability::Blocking,
        );

        am
    }

    async fn create(&self, path: &str, args: OpCreate) -> Result<RpCreate> {
        // self.blocking_create(path, args)

        unimplemented!()
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        self.blocking_read(path, args)
    }

    async fn write(&self, path: &str, args: OpWrite, mut r: Reader) -> Result<RpWrite> {
        let (parent, file_name) = self.split_path(path);

        let tree = self.db.open_tree(parent).unwrap();

        let mut read_vec = Vec::with_capacity(args.size() as usize);
        let bytes_read = r.read_to_end(&mut read_vec).await.unwrap();

        tree.insert(file_name, read_vec).unwrap();

        Ok(RpWrite::new(bytes_read as u64))
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.blocking_stat(path, args)
    }

    async fn delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
        self.blocking_delete(path, args)
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, ObjectPager)> {
        let absolute_path = build_rooted_abs_path(&self.root, path);

        let tree = self.db.open_tree(absolute_path).unwrap();

        Ok((RpList::default(), Box::new(DirPager::new(tree, 256))))
    }

    fn blocking_create(&self, path: &str, args: OpCreate) -> Result<RpCreate> {
        if args.mode() == ObjectMode::FILE {
            let (parent, file_name) = self.split_path(path);

            let tree = self.db.open_tree(parent).unwrap();
            tree.insert(file_name, vec![]).unwrap();
        }

        Ok(RpCreate::default())
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        let (parent, file_name) = self.split_path(path);

        let tree = self.db.open_tree(parent).unwrap();

        let subset = apply_range(tree.get(file_name).unwrap().unwrap().to_vec(), args.range());
        let subset_len = subset.len() as u64;

        let cursor = output::Cursor::from(subset);

        Ok((RpRead::new(subset_len), cursor))
    }

    fn blocking_write(&self, path: &str, args: OpWrite, mut r: BlockingReader) -> Result<RpWrite> {
        let (parent, file_name) = self.split_path(path);

        let tree = self.db.open_tree(parent).unwrap();

        let mut read_vec = Vec::with_capacity(args.size() as usize);
        let bytes_read = r.read_to_end(&mut read_vec).unwrap();

        tree.insert(file_name, read_vec).unwrap();

        Ok(RpWrite::new(bytes_read as u64))
    }

    fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let (parent, file_name) = self.split_path(path);

        let tree = self.db.open_tree(parent).unwrap();
        assert!(tree.contains_key(file_name).unwrap());

        Ok(RpStat::new(ObjectMetadata::new(ObjectMode::FILE)))
    }

    fn blocking_delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
        let (parent, file_name) = self.split_path(path);

        let tree = self.db.open_tree(parent).unwrap();
        tree.remove(file_name).unwrap().unwrap();

        Ok(RpDelete::default())
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, BlockingObjectPager)> {
        let absolute_path = build_rooted_abs_path(&self.root, path);

        let tree = self.db.open_tree(absolute_path).unwrap();

        Ok((RpList::default(), Box::new(DirPager::new(tree, 256))))
    }
}

fn apply_range(mut bs: Vec<u8>, br: BytesRange) -> Vec<u8> {
    match (br.offset(), br.size()) {
        (Some(offset), Some(size)) => {
            let mut bs = bs.split_off(offset as usize);
            if (size as usize) < bs.len() {
                let _ = bs.split_off(size as usize);
            }
            bs
        }
        (Some(offset), None) => bs.split_off(offset as usize),
        (None, Some(size)) => bs.split_off(bs.len() - size as usize),
        (None, None) => bs,
    }
}
