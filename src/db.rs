// Copyright Rivtower Technologies LLC.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use cita_cloud_proto::storage::Regions;
use rocksdb::DB as RocksDB;
use rocksdb::{ColumnFamilyDescriptor, Options};
use std::path::Path;
use std::vec::Vec;
use tonic::Status;

pub struct DB {
    db: RocksDB,
}

impl DB {
    pub fn new(db_path: &str) -> Self {
        let root_path = Path::new(".");
        let path = root_path.join(db_path);

        let mut cfs = Vec::new();
        for i in 0..Regions::Button as u8 {
            let mut cf_opts = Options::default();
            cf_opts.set_max_write_buffer_number(16);
            let cf = ColumnFamilyDescriptor::new(format!("{}", i), cf_opts);
            cfs.push(cf);
        }

        let mut db_opts = Options::default();
        db_opts.create_missing_column_families(true);
        db_opts.create_if_missing(true);

        let db = RocksDB::open_cf_descriptors(&db_opts, path, cfs).unwrap();

        DB { db }
    }

    pub fn store(&self, region: u32, key: Vec<u8>, value: Vec<u8>) -> Result<(), Status> {
        if let Some(cf) = self.db.cf_handle(&format!("{}", region)) {
            let ret = self.db.put_cf(cf, key, value);
            ret.map_err(|e| Status::aborted(format!("store error: {:?}", e)))
        } else {
            Err(Status::aborted("bad region"))
        }
    }

    pub fn load(&self, region: u32, key: Vec<u8>) -> Result<Vec<u8>, Status> {
        if let Some(cf) = self.db.cf_handle(&format!("{}", region)) {
            let ret = self.db.get_cf(cf, key);
            match ret {
                Ok(opt_v) => match opt_v {
                    Some(v) => Ok(v),
                    None => Err(Status::not_found("key not found")),
                },
                Err(e) => Err(Status::aborted(format!("store error: {:?}", e))),
            }
        } else {
            Err(Status::aborted("bad region"))
        }
    }

    pub fn delete(&self, region: u32, key: Vec<u8>) -> Result<(), Status> {
        if let Some(cf) = self.db.cf_handle(&format!("{}", region)) {
            let ret = self.db.delete_cf(cf, key);
            ret.map_err(|e| Status::aborted(format!("store error: {:?}", e)))
        } else {
            Err(Status::aborted("bad region"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DB;
    use quickcheck::quickcheck;
    use quickcheck::Arbitrary;
    use quickcheck::Gen;
    use tempfile::tempdir;

    #[derive(Clone, Debug)]
    struct DBTestArgs {
        region: u32,
        key: Vec<u8>,
        value: Vec<u8>,
    }

    impl Arbitrary for DBTestArgs {
        fn arbitrary(g: &mut Gen) -> Self {
            let region = u32::arbitrary(g) % 10;
            let key = match region {
                1 | 7 | 8 | 9 => {
                    let mut k = Vec::with_capacity(32);
                    for _ in 0..4 {
                        let bytes = u64::arbitrary(g).to_be_bytes().to_vec();
                        k.extend_from_slice(&bytes);
                    }
                    k
                }
                _ => {
                    let mut k = Vec::with_capacity(8);
                    let bytes = u64::arbitrary(g).to_be_bytes().to_vec();
                    k.extend_from_slice(&bytes);
                    k
                }
            };

            let value = match region {
                7 | 8 | 9 => {
                    let mut v = Vec::with_capacity(8);
                    let bytes = u64::arbitrary(g).to_be_bytes().to_vec();
                    v.extend_from_slice(&bytes);
                    v
                }
                _ => {
                    let mut v = Vec::with_capacity(32);
                    for _ in 0..4 {
                        let bytes = u64::arbitrary(g).to_be_bytes().to_vec();
                        v.extend_from_slice(&bytes);
                    }
                    v
                }
            };

            DBTestArgs { region, key, value }
        }
    }

    quickcheck! {
         fn prop(args: DBTestArgs) -> bool {
            let dir = tempdir().unwrap();
             let path = dir.path().to_str().unwrap();
             let db = DB::new(path);

             let region = args.region;
             let key = args.key.clone();
             let value = args.value;

             db.store(region, key.clone(), value.clone()).unwrap();
             db.load(region, key).unwrap() == value
         }
    }
}
