#!/bin/sh

export SQLX_OFFLINE=true
cargo \
	check
