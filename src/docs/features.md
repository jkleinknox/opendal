default feature is `rustls` which enable rustls support.

## Layer Features

- `layers-all`: Enable all layers support.
- `layers-metrics`: Enable metrics layer support.
- `layers-tracing`: Enable tracing layer support.
- `layers-chaos`: Enable chaos layer support.

## Service Features

- `services-ftp`: Enable ftp service support.
- `services-hdfs`: Enable hdfs service support.
- `services-moka`: Enable moka service support.
- `services-ipfs`: Enable ipfs service support.
- `services-redis`: Enable redis service support.
- `services-rocksdb`: Enable rocksdb service support.
- `services-sled`: Enable sled service support.

## Dependencies Features

- `compress`: Enable object decompress read support.
- `rustls`: Enable TLS functionality provided by `rustls`, enabled by default
- `native-tls`: Enable TLS functionality provided by `native-tls`
- `native-tls-vendored`: Enable the `vendored` feature of `native-tls`
