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

use crate::util::{
    check_key, check_region, check_value, full_to_compact, get_block_hash, get_tx_hash,
};
use cita_cloud_proto::{
    blockchain::{Block, CompactBlock, RawTransaction, RawTransactions},
    storage::Regions,
};
use prost::Message;
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
        for i in 0..Regions::FullBlock as u8 {
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
        if !check_region(region) {
            return Err(Status::invalid_argument("invalid region"));
        }

        if !check_key(region, &key) {
            return Err(Status::invalid_argument("invalid key"));
        }

        if !check_value(region, &value) {
            return Err(Status::invalid_argument("invalid value"));
        }

        if let Some(cf) = self.db.cf_handle(&format!("{}", region)) {
            let ret = self.db.put_cf(cf, key, value);
            ret.map_err(|e| Status::aborted(format!("store error: {:?}", e)))
        } else {
            Err(Status::aborted("bad region"))
        }
    }

    pub fn load(&self, region: u32, key: Vec<u8>) -> Result<Vec<u8>, Status> {
        if !check_region(region) {
            return Err(Status::invalid_argument("invalid region"));
        }

        if !check_key(region, &key) {
            return Err(Status::invalid_argument("invalid key"));
        }

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
        if !check_region(region) {
            return Err(Status::invalid_argument("invalid region"));
        }

        if !check_key(region, &key) {
            return Err(Status::invalid_argument("invalid key"));
        }

        if let Some(cf) = self.db.cf_handle(&format!("{}", region)) {
            let ret = self.db.delete_cf(cf, key);
            ret.map_err(|e| Status::aborted(format!("store error: {:?}", e)))
        } else {
            Err(Status::aborted("bad region"))
        }
    }

    pub fn store_full_block(
        &self,
        height_bytes: Vec<u8>,
        block_bytes: Vec<u8>,
    ) -> Result<(), Status> {
        if !check_key(11, &height_bytes) {
            return Err(Status::invalid_argument("invalid key"));
        }

        let block = Block::decode(block_bytes.as_slice())
            .map_err(|_| Status::invalid_argument("decode Block failed"))?;

        let block_hash = get_block_hash(block.header.as_ref())?;

        for (tx_index, raw_tx) in block
            .body
            .clone()
            .ok_or(Status::invalid_argument("block body not found"))?
            .body
            .into_iter()
            .enumerate()
        {
            let mut tx_bytes = Vec::new();
            raw_tx
                .encode(&mut tx_bytes)
                .map_err(|_| Status::invalid_argument("encode RawTransaction failed"))?;

            let tx_hash =
                get_tx_hash(&raw_tx).ok_or(Status::invalid_argument("can not get tx hash"))?;
            self.store(1, tx_hash.clone(), tx_bytes)?;
            self.store(7, tx_hash.clone(), height_bytes.clone())?;
            self.store(9, tx_hash, tx_index.to_be_bytes().to_vec())?;
        }

        self.store(4, height_bytes.clone(), block_hash.clone())?;
        self.store(8, block_hash, height_bytes.clone())?;
        self.store(5, height_bytes.clone(), block.proof.clone())?;

        let compact_block = full_to_compact(block);
        let mut compact_block_bytes = Vec::new();
        compact_block
            .encode(&mut compact_block_bytes)
            .map_err(|_| Status::invalid_argument("encode CompactBlock failed"))?;
        self.store(10, height_bytes, compact_block_bytes)?;

        Ok(())
    }

    pub fn load_full_block(&self, height_bytes: Vec<u8>) -> Result<Vec<u8>, Status> {
        let compact_block_bytes = self.load(10, height_bytes.clone())?;
        let compact_block = CompactBlock::decode(compact_block_bytes.as_slice())
            .map_err(|_| Status::invalid_argument("decode CompactBlock failed"))?;

        let proof = self.load(5, height_bytes)?;
        let block = self.get_full_block(compact_block, proof)?;

        let mut block_bytes = Vec::new();
        block
            .encode(&mut block_bytes)
            .map_err(|_| Status::invalid_argument("encode Block failed"))?;

        Ok(block_bytes)
    }

    pub fn get_full_block(
        &self,
        compact_block: CompactBlock,
        proof: Vec<u8>,
    ) -> Result<Block, Status> {
        let mut body = Vec::new();
        if let Some(compact_body) = compact_block.body {
            for tx_hash in compact_body.tx_hashes {
                let tx_bytes = self.load(1, tx_hash)?;
                let raw_tx = RawTransaction::decode(tx_bytes.as_slice())
                    .map_err(|_| Status::invalid_argument("decode RawTransaction failed"))?;
                body.push(raw_tx)
            }
        }

        Ok(Block {
            version: compact_block.version,
            header: compact_block.header,
            body: Some(RawTransactions { body }),
            proof,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cita_cloud_proto::blockchain::{
        raw_transaction::Tx, BlockHeader, Transaction, UnverifiedTransaction, Witness,
    };
    use minitrace::*;
    use minitrace_jaeger::Reporter;
    use minitrace_macro::trace;
    use quickcheck::quickcheck;
    use quickcheck::Arbitrary;
    use quickcheck::Gen;
    use rand::{thread_rng, Rng};
    use std::net::SocketAddr;
    use std::time::Instant;
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
                1 | 7 | 8 | 9 | 10 => {
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

    fn build_normal_tx(to: Vec<u8>, data: Vec<u8>, value: Vec<u8>) -> Transaction {
        // get start block number
        let nonce = rand::random::<u64>().to_string();
        Transaction {
            version: 0,
            to,
            nonce,
            quota: 3_000_000,
            valid_until_block: 0,
            data,
            value,
            chain_id: vec![0; 32],
        }
    }

    fn prepare_raw_tx(tx: Transaction) -> RawTransaction {
        // calc tx hash
        let tx_hash = {
            // build tx bytes
            let tx_bytes = {
                let mut buf = Vec::with_capacity(tx.encoded_len());
                tx.encode(&mut buf).unwrap();
                buf
            };
            hash_data(tx_bytes.as_slice())
        };

        // build raw tx
        let raw_tx = {
            let witness = Witness {
                signature: vec![0; 64],
                sender: vec![0; 20],
            };

            let unverified_tx = UnverifiedTransaction {
                transaction: Some(tx),
                transaction_hash: tx_hash,
                witness: Some(witness),
            };

            RawTransaction {
                tx: Some(Tx::NormalTx(unverified_tx)),
            }
        };

        raw_tx
    }

    #[trace("create_full_block")]
    fn create_full_block(tx_num: u64, height: u64) -> (Vec<u8>, Vec<u8>) {
        let default_proof = vec![0; 128];
        let mut rng = thread_rng();
        let mut body = Vec::new();

        for _ in 0..tx_num {
            let to: [u8; 20] = rng.gen();
            let data: [u8; 32] = rng.gen();
            let value: [u8; 32] = rng.gen();

            let tx = build_normal_tx(to.to_vec(), data.to_vec(), value.to_vec());
            let raw_tx = prepare_raw_tx(tx);
            body.push(raw_tx);
        }

        let d = ::std::time::UNIX_EPOCH.elapsed().unwrap();

        let block = Block {
            version: 0,
            header: Some(BlockHeader {
                prevhash: vec![0; 32],
                timestamp: d.as_secs() * 1_000 + u64::from(d.subsec_millis()),
                height,
                transactions_root: vec![0; 32],
                proposer: vec![0; 32],
            }),
            body: Some(RawTransactions { body }),
            proof: default_proof,
        };

        let block_bytes = {
            let mut buf = Vec::new();
            block.encode(&mut buf).unwrap();
            buf
        };

        (height.to_be_bytes().to_vec(), block_bytes)
    }

    #[test]
    fn full_block_store_load_test() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let db = DB::new(path);
        let h = 0;

        let (block_hash, block_bytes) = create_full_block(6000, h);

        db.store_full_block(block_hash.clone(), block_bytes.clone())
            .unwrap();
        let load_bytes = db.load_full_block(block_hash).unwrap();

        assert_eq!(block_bytes, load_bytes)
    }

    #[test]
    fn full_block_store_bench_test() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let db = DB::new(path);
        let mut h = 0;

        let now = Instant::now();
        let spans = {
            let (root_span, collector) = Span::root("root");
            let _span_guard = root_span.enter();

            let _local_span_guard = LocalSpan::enter("child");

            for _ in 0..5 {
                let (height_bytes, block_bytes) = create_full_block(6000, h);

                let _ = db.store_full_block(height_bytes, block_bytes);
                h = h + 1;
            }

            collector
        }
        .collect();

        // Report to Jaeger
        let socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 6831);

        const TRACE_ID: u64 = 42;
        const SPAN_ID_PREFIX: u32 = 42;
        const ROOT_PARENT_SPAN_ID: u64 = 0;
        let bytes = Reporter::encode(
            String::from("full_block_store_bench_test"),
            TRACE_ID,
            ROOT_PARENT_SPAN_ID,
            SPAN_ID_PREFIX,
            &spans,
        )
        .expect("encode error");
        Reporter::report(socket, &bytes).expect("report error");

        println!("{:?}", now.elapsed());
    }
}
