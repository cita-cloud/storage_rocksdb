# storage_rocksdb

`CITA-Cloud`中[storage微服务](https://github.com/cita-cloud/cita_cloud_proto/blob/master/protos/storage.proto)的实现，基于[rocksdb](https://github.com/facebook/rocksdb)。

## 编译docker镜像
```
docker build -t citacloud/storage_rocksdb .
```

## 使用方法

```
$ storage -h
storage service

Usage: storage <COMMAND>

Commands:
  run   run this service
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### storage-run

运行`storage`服务。

```
$ storage run -h
run this service

Usage: storage run [OPTIONS]

Options:
  -c, --config <CONFIG_PATH>  Chain config path [default: config.toml]
  -h, --help                  Print help
```

参数：
1. `config` 微服务配置文件。

    参见示例`example/config.toml`。

    其中`[storage_rocksdb]`段为微服务的配置：
    * `crypto_port` 为依赖的`Crypto`微服务的`gRPC`服务监听的端口号。
    * `storage_port` 为本微服务的`gRPC`服务监听的端口号。
    * `write_buffer_size` 设置`rocksdb`的写缓存大小，单位为字节。
    * `max_open_file` 设置`rocksdb`最大打开文件数量。
    * `domain` 节点的域名

    其中`[storage_rocksdb.log_config]`段为微服务日志的配置：
    * `max_level` 日志等级
    * `filter` 日志过滤配置
    * `service_name` 服务名称，用作日志文件名与日志采集的服务名称
    * `rolling_file_path` 日志文件路径
    * `agent_endpoint` jaeger 采集端地址

```
$ storage run -c example/config.toml
2023-02-09T02:22:10.237028Z  INFO storage: grpc port of storage_rocksdb: 60003
2023-02-09T02:22:10.237141Z  INFO storage: db path of this service: chain_data
2023-02-09T02:22:10.534378Z  INFO storage: start storage_rocksdb grpc server
```

## 设计


