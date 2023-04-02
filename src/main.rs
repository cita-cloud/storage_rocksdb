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

mod config;
mod db;
mod health_check;
mod panic_hook;
mod util;

#[macro_use]
extern crate tracing;

use crate::panic_hook::set_panic_handler;
use crate::util::clap_about;
use clap::Parser;

#[derive(Parser)]
#[clap(version, about = clap_about())]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Parser)]
enum SubCommand {
    /// run this service
    #[clap(name = "run")]
    Run(RunOpts),
}

/// A subcommand for run
#[derive(Parser)]
struct RunOpts {
    /// Chain config path
    #[clap(short = 'c', long = "config", default_value = "config.toml")]
    config_path: String,
}

fn main() {
    ::std::env::set_var("RUST_BACKTRACE", "full");
    set_panic_handler();

    let opts: Opts = Opts::parse();

    match opts.subcmd {
        SubCommand::Run(opts) => {
            let fin = run(opts);
            warn!("Should not reach here {:?}", fin);
        }
    }
}

use crate::config::StorageConfig;
use crate::health_check::HealthCheckServer;
use crate::util::init_grpc_client;
use cita_cloud_proto::common::StatusCode;
use cita_cloud_proto::health_check::health_server::HealthServer;
use cita_cloud_proto::status_code::StatusCodeEnum;
use cita_cloud_proto::storage::{
    storage_service_server::StorageService, storage_service_server::StorageServiceServer, Content,
    ExtKey, Value,
};
use cloud_util::metrics::{run_metrics_exporter, MiddlewareLayer};
use db::DB;
use std::net::AddrParseError;
use std::path::Path;
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};

pub struct StorageServer {
    db: Arc<DB>,
}

impl StorageServer {
    fn new(db: Arc<DB>) -> Self {
        StorageServer { db }
    }
}

#[tonic::async_trait]
impl StorageService for StorageServer {
    #[instrument(skip_all)]
    async fn store(&self, request: Request<Content>) -> Result<Response<StatusCode>, Status> {
        cloud_util::tracer::set_parent(&request);
        debug!("store request: {:?}", request);

        let content = request.into_inner();
        let region = content.region;
        let key = content.key;
        let value = content.value;

        if region == 12 {
            match self.db.store_all_block_data(key, value).await {
                Ok(()) => Ok(Response::new(StatusCodeEnum::Success.into())),
                Err(status) => {
                    warn!("store_all_block_data failed: {}", status.to_string());
                    Ok(Response::new(status.into()))
                }
            }
        } else {
            match self.db.store(region, key, value) {
                Ok(()) => Ok(Response::new(StatusCodeEnum::Success.into())),
                Err(status) => {
                    warn!("store failed: {}", status.to_string());
                    Ok(Response::new(status.into()))
                }
            }
        }
    }

    #[instrument(skip_all)]
    async fn load(&self, request: Request<ExtKey>) -> Result<Response<Value>, Status> {
        cloud_util::tracer::set_parent(&request);
        debug!("load request: {:?}", request);

        let ext_key = request.into_inner();
        let region = ext_key.region;
        let key = ext_key.key;

        if region == 11 {
            match self.db.load_full_block(key) {
                Ok(value) => Ok(Response::new(Value {
                    status: Some(StatusCodeEnum::Success.into()),
                    value,
                })),
                Err(status) => {
                    warn!("load_full_block failed: {}", status.to_string());
                    Ok(Response::new(Value {
                        status: Some(status.into()),
                        value: vec![],
                    }))
                }
            }
        } else if key == 1u64.to_be_bytes().to_vec() && region == 0 {
            match self.db.load(region, 0u64.to_be_bytes().to_vec()) {
                Ok(height) => match self.db.load(4, height) {
                    Ok(value) => Ok(Response::new(Value {
                        status: Some(StatusCodeEnum::Success.into()),
                        value,
                    })),
                    Err(status) => {
                        warn!("load failed: {}", status.to_string());
                        Ok(Response::new(Value {
                            status: Some(status.into()),
                            value: vec![],
                        }))
                    }
                },
                Err(status) => {
                    warn!("load failed: {}", status.to_string());
                    Ok(Response::new(Value {
                        status: Some(status.into()),
                        value: vec![],
                    }))
                }
            }
        } else {
            match self.db.load(region, key) {
                Ok(value) => Ok(Response::new(Value {
                    status: Some(StatusCodeEnum::Success.into()),
                    value,
                })),
                Err(status) => {
                    warn!("load failed: {}", status.to_string());
                    Ok(Response::new(Value {
                        status: Some(status.into()),
                        value: vec![],
                    }))
                }
            }
        }
    }

    #[instrument(skip_all)]
    async fn delete(&self, request: Request<ExtKey>) -> Result<Response<StatusCode>, Status> {
        cloud_util::tracer::set_parent(&request);
        debug!("delete request: {:?}", request);

        let ext_key = request.into_inner();
        let region = ext_key.region;
        let key = ext_key.key;

        match self.db.delete(region, key) {
            Ok(()) => Ok(Response::new(StatusCodeEnum::Success.into())),
            Err(status) => {
                warn!("delete error: {}", status.to_string());
                Ok(Response::new(status.into()))
            }
        }
    }
}

#[tokio::main]
async fn run(opts: RunOpts) -> Result<(), StatusCodeEnum> {
    tokio::spawn(cloud_util::signal::handle_signals());

    let config = StorageConfig::new(&opts.config_path);
    init_grpc_client(&config);

    // init tracer
    cloud_util::tracer::init_tracer(config.domain.clone(), &config.log_config)
        .map_err(|e| println!("tracer init err: {e}"))
        .unwrap();

    info!("grpc port of storage_rocksdb: {}", &config.storage_port);

    // db_path must be relative path
    assert!(
        !Path::new(&config.db_path).is_absolute(),
        "db_path must be relative path"
    );
    info!("db path of this service: {}", &config.db_path);

    let addr_str = format!("127.0.0.1:{}", config.storage_port);
    let addr = addr_str.parse().map_err(|e: AddrParseError| {
        warn!("grpc listen addr parse failed: {} ", e);
        StatusCodeEnum::FatalError
    })?;

    // init db
    let db = Arc::new(DB::new(&config.db_path, &config));
    let storage_server = StorageServer::new(db.clone());

    let layer = if config.enable_metrics {
        tokio::spawn(async move {
            run_metrics_exporter(config.metrics_port).await.unwrap();
        });

        Some(
            tower::ServiceBuilder::new()
                .layer(MiddlewareLayer::new(config.metrics_buckets))
                .into_inner(),
        )
    } else {
        None
    };

    info!("start storage_rocksdb grpc server");
    if layer.is_some() {
        info!("metrics on");
        Server::builder()
            .layer(layer.unwrap())
            .add_service(StorageServiceServer::new(storage_server))
            .add_service(HealthServer::new(HealthCheckServer::new(db)))
            .serve(addr)
            .await
            .map_err(|e| {
                warn!(
                    "start storage_rocksdb grpc server failed: {} ",
                    e.to_string()
                );
                StatusCodeEnum::FatalError
            })?;
    } else {
        info!("metrics off");
        Server::builder()
            .add_service(StorageServiceServer::new(storage_server))
            .add_service(HealthServer::new(HealthCheckServer::new(db)))
            .serve(addr)
            .await
            .map_err(|e| {
                warn!(
                    "start storage_rocksdb grpc server failed: {} ",
                    e.to_string()
                );
                StatusCodeEnum::FatalError
            })?;
    }

    Ok(())
}
