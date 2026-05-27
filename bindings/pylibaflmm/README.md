# How to use python bindings

## First time setup

```bash
# Navigate to the pylibaflmm directory
cd LibAFLmm/bindings/pylibaflmm
# Create virtual environment
python3 -m venv .env
# Activate virtual environment
source .env/bin/activate
# Install maturin
pip install maturin
# Build python module
maturin develop
```

This is going to install `pylibaflmm` python module into this venv.

## Use bindings

### Example: Running baby_fuzzer in fuzzers/baby_fuzzer/baby_fuzzer.py

First, make sure the python virtual environment is activated. If not, run `source .env/bin/activate
`. Running `pip freeze` at this point should display the following (versions may differ):

```ini
maturin==0.12.6
pylibaflmm==0.7.0
toml==0.10.2
```

Then simply run

```sh
python PATH_TO_BABY_FUZZER/baby_fuzzer.py
```

The crashes directory will be created in the directory from which you ran the command.
