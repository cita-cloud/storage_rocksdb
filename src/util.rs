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

use cita_cloud_proto::blockchain::BlockHeader;
use cita_cloud_proto::{
    blockchain::{raw_transaction::Tx, Block, CompactBlock, CompactBlockBody, RawTransaction},
    storage::Regions,
};
use prost::Message;
use tonic::Status;

pub fn check_region(region: u32) -> bool {
    region < Regions::Button as u8 as u32
}

pub fn check_key(region: u32, key: &[u8]) -> bool {
    match region {
        1 | 7 | 8 | 9 => key.len() == 32,
        _ => key.len() == 8,
    }
}

pub fn check_value(region: u32, value: &[u8]) -> bool {
    match region {
        4 | 6 => value.len() == 32,
        7 | 8 => value.len() == 8,
        _ => true,
    }
}

pub fn get_tx_hash(raw_tx: &RawTransaction) -> Option<Vec<u8>> {
    match raw_tx.tx {
        Some(Tx::NormalTx(ref normal_tx)) => Some(normal_tx.transaction_hash.clone()),
        Some(Tx::UtxoTx(ref utxo_tx)) => Some(utxo_tx.transaction_hash.clone()),
        None => None,
    }
}

pub fn full_to_compact(block: Block) -> CompactBlock {
    let mut compact_body = CompactBlockBody { tx_hashes: vec![] };

    if let Some(body) = block.body {
        for raw_tx in body.body {
            match raw_tx.tx {
                Some(Tx::NormalTx(normal_tx)) => {
                    compact_body.tx_hashes.push(normal_tx.transaction_hash)
                }
                Some(Tx::UtxoTx(utxo_tx)) => compact_body.tx_hashes.push(utxo_tx.transaction_hash),
                None => {}
            }
        }
    }

    CompactBlock {
        version: block.version,
        header: block.header,
        body: Some(compact_body),
    }
}

pub fn hash_data(data: &[u8]) -> Vec<u8> {
    libsm::sm3::hash::Sm3Hash::new(data).get_hash().to_vec()
}

pub fn get_block_hash(header: Option<&BlockHeader>) -> Result<Vec<u8>, Status> {
    match header {
        Some(header) => {
            let mut block_header_bytes = Vec::new();
            header
                .encode(&mut block_header_bytes)
                .map_err(|_| Status::invalid_argument("encode block header failed"))?;
            let block_hash = hash_data(&block_header_bytes);
            Ok(block_hash)
        }

        None => return Err(Status::invalid_argument("no blockheader")),
    }
}
