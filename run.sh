#!/bin/sh

export PGUSER=postgres
export ENV_PG_CONN_STR=postgresql://127.0.0.1:5433/postgres

export ENV_WD_LISTEN_PORT=61388

./rs-pgdbsize2webdav
