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

mod db;

use clap::Clap;
use git_version::git_version;
use log::{debug, info, warn};

const GIT_VERSION: &str = git_version!(
    args = ["--tags", "--always", "--dirty=-modified"],
    fallback = "unknown"
);
const GIT_HOMEPAGE: &str = "https://github.com/cita-cloud/storage_rocksdb";

/// network service
#[derive(Clap)]
#[clap(version = "0.1.0", author = "Rivtower Technologies.")]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    /// print information from git
    #[clap(name = "git")]
    GitInfo,
    /// run this service
    #[clap(name = "run")]
    Run(RunOpts),
}

/// A subcommand for run
#[derive(Clap)]
struct RunOpts {
    /// Sets grpc port of this service.
    #[clap(short = 'p', long = "port", default_value = "50003")]
    grpc_port: String,
    /// Sets db path.
    #[clap(short = 'd', long = "db", default_value = "chain_data")]
    db_path: String,
}

fn main() {
    ::std::env::set_var("RUST_BACKTRACE", "full");

    let opts: Opts = Opts::parse();

    match opts.subcmd {
        SubCommand::GitInfo => {
            println!("git version: {}", GIT_VERSION);
            println!("homepage: {}", GIT_HOMEPAGE);
        }
        SubCommand::Run(opts) => {
            // init log4rs
            log4rs::init_file("storage-log4rs.yaml", Default::default()).unwrap();
            info!("grpc port of this service: {}", opts.grpc_port);
            info!("db path of this service: {}", opts.db_path);
            let _ = run(opts);
        }
    }
}

use std::path::Path;
use tokio::fs;

async fn get_tx(tx_hash: &[u8]) -> Option<Vec<u8>> {
    if tx_hash.len() != 32 {
        warn!("len of tx_hash is not correct");
        return None;
    }
    let filename = hex::encode(tx_hash);
    let root_path = Path::new(".");
    let tx_path = root_path.join("txs").join(filename);

    fs::read(tx_path).await.ok()
}

use cita_cloud_proto::common::SimpleResponse;
use cita_cloud_proto::storage::{
    storage_service_server::StorageService, storage_service_server::StorageServiceServer, Content,
    ExtKey, Value,
};
use tonic::{transport::Server, Request, Response, Status};

use db::DB;

pub struct StorageServer {
    db: DB,
}

impl StorageServer {
    fn new(db: DB) -> Self {
        StorageServer { db }
    }
}

#[tonic::async_trait]
impl StorageService for StorageServer {
    async fn store(&self, request: Request<Content>) -> Result<Response<SimpleResponse>, Status> {
        debug!("store request: {:?}", request);

        let content = request.into_inner();
        let region = content.region;
        let key = content.key;
        let value = content.value;

        let ret = self.db.store(region, key, value);
        if ret.is_err() {
            warn!("store error: {:?}", ret);
            Err(Status::aborted("db store failed"))
        } else {
            let reply = SimpleResponse { is_success: true };
            Ok(Response::new(reply))
        }
    }

    async fn load(&self, request: Request<ExtKey>) -> Result<Response<Value>, Status> {
        debug!("load request: {:?}", request);

        let ext_key = request.into_inner();
        let region = ext_key.region;
        let key = ext_key.key;

        let ret = if region == 1 {
            get_tx(&key).await.ok_or_else(|| "get tx failed".to_owned())
        } else {
            self.db.load(region, key)
        };
        if let Ok(value) = ret {
            let reply = Value { value };
            Ok(Response::new(reply))
        } else {
            warn!("load error: {:?}", ret);
            Err(Status::aborted("db load failed"))
        }
    }

    async fn delete(&self, request: Request<ExtKey>) -> Result<Response<SimpleResponse>, Status> {
        debug!("delete request: {:?}", request);

        let ext_key = request.into_inner();
        let region = ext_key.region;
        let key = ext_key.key;

        let ret = self.db.delete(region, key);
        if ret.is_err() {
            warn!("delete error: {:?}", ret);
            Err(Status::aborted("db delete failed"))
        } else {
            let reply = SimpleResponse { is_success: true };
            Ok(Response::new(reply))
        }
    }
}

#[tokio::main]
async fn run(opts: RunOpts) -> Result<(), Box<dyn std::error::Error>> {
    let addr_str = format!("127.0.0.1:{}", opts.grpc_port);
    let addr = addr_str.parse()?;

    // init db
    let db = DB::new(&opts.db_path);
    let storage_server = StorageServer::new(db);

    Server::builder()
        .add_service(StorageServiceServer::new(storage_server))
        .serve(addr)
        .await?;

    Ok(())
}
