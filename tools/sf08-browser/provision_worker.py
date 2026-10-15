"""Dedicated owned trust subprocess; only C3's guarded provisioning mutates stores."""
import argparse
import json
import os
from pathlib import Path
import provision


def run(work):
    os.umask(0o077)
    trust = provision.provision(work / 'ca-for-trust.pem')
    # This file is produced by C3 under a new owned0700 trust directory.
    data = (trust / 'provisioned.json').read_bytes()
    if len(data) > 4096:
        raise ValueError('receipt bound')
    # Do not print paths or raw data. Parent independently validates every field.
    provision.exclusive(work / 'trust.json', data)


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--work', type=Path, required=True)
    args = p.parse_args()
    try:
        run(args.work)
    except BaseException:
        raise SystemExit(1) from None
