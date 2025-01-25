#!/usr/bin/env bash

python3 -m venv venv
./venv/bin/pip3 install -r ./music21/requirements.txt
./venv/bin/python3 -m generate_tables
cargo fmt
