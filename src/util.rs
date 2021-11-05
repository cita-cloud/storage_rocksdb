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

use crate::config::StorageConfig;
use cita_cloud_proto::kms::kms_service_client::KmsServiceClient;
use cita_cloud_proto::{
    blockchain::{raw_transaction::Tx, Block, CompactBlock, CompactBlockBody},
    storage::Regions,
};
use tokio::sync::OnceCell;
use tonic::transport::{Channel, Endpoint};

pub static KMS_CLIENT: OnceCell<KmsServiceClient<Channel>> = OnceCell::const_new();

// This must be called before access to clients.
pub fn init_grpc_client(config: &StorageConfig) {
    KMS_CLIENT
        .set({
            let addr = format!("http://127.0.0.1:{}", config.kms_port);
            let channel = Endpoint::from_shared(addr).unwrap().connect_lazy().unwrap();
            KmsServiceClient::new(channel)
        })
        .unwrap();
}

pub fn kms_client() -> KmsServiceClient<Channel> {
    KMS_CLIENT.get().cloned().unwrap()
}

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
