#! /usr/bin/env bash

cd lopez
cargo build --release --target x86_64-unknown-linux-musl
cd ..
cd entalator
cargo build --release --target x86_64-unknown-linux-musl
cd ..
