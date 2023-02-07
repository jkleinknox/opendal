use crate::raw::{BlockingObjectPage, ObjectEntry, ObjectPage};
use crate::{ObjectMetadata, ObjectMode, Result};
use async_trait::async_trait;
use sled;
use std::ops::Deref;

pub struct DirPager {
    current_page: usize,
    page_size: usize,
    db_tree: sled::Tree,
}

impl DirPager {
    pub fn new(tree: sled::Tree, page_size: usize) -> Self {
        DirPager {
            current_page: 0,
            page_size,
            db_tree: tree,
        }
    }
}

#[async_trait]
impl ObjectPage for DirPager {
    async fn next_page(&mut self) -> Result<Option<Vec<ObjectEntry>>> {
        <Self as BlockingObjectPage>::next_page(self)
    }
}

impl BlockingObjectPage for DirPager {
    fn next_page(&mut self) -> Result<Option<Vec<ObjectEntry>>> {
        let num_total_items = self.db_tree.len();
        if self.current_page * self.page_size > num_total_items {
            return Ok(None);
        }

        let name = String::from_utf8(self.db_tree.name().to_vec()).unwrap();

        let objects = self
            .db_tree
            .iter()
            .skip(self.current_page * self.page_size)
            .filter_map(|res| match res {
                Ok((k, _)) => {
                    let file_name = String::from_utf8(k.to_vec()).unwrap();
                    Some(ObjectEntry::new(
                        &format!("{name}/{file_name}"),
                        ObjectMetadata::new(ObjectMode::FILE),
                    ))
                }
                Err(_) => None,
            })
            .collect::<Vec<ObjectEntry>>();

        self.current_page += 1;

        Ok(Some(objects))
    }
}
