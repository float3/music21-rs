#!/usr/bin/env nix-shell
#!nix-shell -i bash -p git python312 python312Packages.virtualenv rustfmt

set -e

activate_venv() {
  if [ -f "./venv/bin/activate" ]; then
    source ./venv/bin/activate
  elif [ -f "./venv/Scripts/activate" ]; then
    source ./venv/Scripts/activate
  else
    python3.12 -m venv venv
    if [ -f "./venv/bin/activate" ]; then
      source ./venv/bin/activate
    elif [ -f "./venv/Scripts/activate" ]; then
      source ./venv/Scripts/activate
    else
      echo "Failed to activate virtual environment."
      exit 1
    fi
  fi
}

git submodule update --init --recursive

cd ./music21; git pull origin master; cd ..

activate_venv

pip install --upgrade pip
pip install -r ./music21/requirements.txt

python -m generate_tables

rustfmt ./src/chord/tables/generated.rs

echo "Setup completed successfully!"

