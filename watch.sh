#!/bin/sh

export SQLX_OFFLINE=true
cargo \
	watch \
	--watch src \
	--watch Cargo.toml \
	--shell ./check.sh
