#!/bin/sh

export SQLX_OFFLINE=true
cargo \
	clippy \
	--all-targets
