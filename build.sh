#!/bin/sh

export SQLX_OFFLINE=true
cargo \
	build \
	--release
