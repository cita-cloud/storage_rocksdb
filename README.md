# storage_rocksdb

`CITA-Cloud`中[storage微服务](https://github.com/cita-cloud/cita_cloud_proto/blob/master/protos/storage.proto)的实现，基于[rocksdb](https://github.com/facebook/rocksdb)。

## 编译docker镜像
```
docker build -t citacloud/storage_rocksdb .
```

## 使用方法

```
$ storage -h
storage 6.4.0
Rivtower Technologies <contact@rivtower.com>
network service

USAGE:
    storage <SUBCOMMAND>

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

SUBCOMMANDS:
    help    Print this message or the help of the given subcommand(s)
    run     run this service
```

### storage-run

运行`storage`服务。

```
$ storage run -h
storage-run
run this service

USAGE:
    storage run [OPTIONS]

OPTIONS:
    -c, --config <CONFIG_PATH>    Chain config path [default: config.toml]
    -h, --help                    Print help information
    -l, --log <LOG_FILE>          log config path [default: storage-log4rs.yaml]
```

参数：
1. `config` 微服务配置文件。

    参见示例`example/config.toml`。

    其中：
    * `crypto_port` 为依赖的`Crypto`微服务的`gRPC`服务监听的端口号。
    * `storage_port` 为本微服务的`gRPC`服务监听的端口号。
    * `write_buffer_size` 设置`rocksdb`的写缓存大小，单位为字节。
    * `max_open_file` 设置`rocksdb`最大打开文件数量。
2. 日志配置文件。

    参见示例`storage-log4rs.yaml`。

    其中：

    * `level` 为日志等级。可选项有：`Error`，`Warn`，`Info`，`Debug`，`Trace`，默认为`Info`。
    * `appenders` 为输出选项，类型为一个数组。可选项有：标准输出(`stdout`)和滚动的日志文件（`journey-service`），默认为同时输出到两个地方。


```
$ storage run -c example/config.toml -l storage-log4rs.yaml
2022-03-14T06:13:17.772131713+00:00 INFO storage - grpc port of this service: 60003
2022-03-14T06:13:17.772278777+00:00 INFO storage - db path of this service: chain_data
```

## 设计


