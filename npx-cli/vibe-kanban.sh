#!/bin/bash

export PORT=3001
export HOST=0.0.0.0
export RUST_LOG=debug

node bin/cli.js
