# muuf

A cli tool to track resource site and send message to download tool.

## supported resource site
dmhy

## supported download tool
transmission

## install
from source, rust toolchain is needed
```
git clone https://github.com/ricardollu/muuf.git
cd muuf
cp config-example.toml config.toml
cargo build --release
```

## usage
check comments in config.toml and
```
./target/release/muuf --help
```