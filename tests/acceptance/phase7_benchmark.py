#!/usr/bin/env python3
"""Measure first, without numeric budget gates. Linux; built release example required.
Every invocation creates a new output directory; raw JSONL and stderr survive failures.
"""
import argparse
from collections import Counter, defaultdict
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import threading
import time

ROOT = Path(__file__).resolve().parents[2]


def command(*args):
    p = subprocess.run(args, text=True, capture_output=True, cwd=ROOT, check=False)
    return {"command": list(args), "returncode": p.returncode,
            "stdout": p.stdout.strip(), "stderr": p.stderr.strip()}


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def load_sample():
    return {"utc": datetime.now(timezone.utc).isoformat(),
            "loadavg": Path('/proc/loadavg').read_text().strip(),
            "memory": [s for s in Path('/proc/meminfo').read_text().splitlines()
                       if s.startswith(('MemAvailable:', 'SwapFree:', 'Dirty:'))],
            # Process names only: do not publish arbitrary application argv/secrets.
            "top_processes_lifetime_cpu": command('ps', '-eo', 'pid,comm,pcpu', '--sort=-pcpu')['stdout'].splitlines()[:16],
            "cpu_pressure": Path('/proc/pressure/cpu').read_text().strip()}


def summarize(records):
    values = defaultdict(list)
    for r in records:
        if r['kind'] == 'sample':
            values[r['metric']].append(r['ms'])
    return {metric: {"n": len(v), "p50_ms": sorted(v)[math.ceil(len(v)*.50)-1],
                     "p95_ms": sorted(v)[math.ceil(len(v)*.95)-1], "min_ms": min(v),
                     "max_ms": max(v)} for metric, v in values.items()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--base', type=Path, required=True, help='existing parent for disposable corpus/state')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    base = args.base.resolve(strict=True)
    binary = ROOT / 'target/release/examples/phase7_benchmark'
    environment = {
        "utc": datetime.now(timezone.utc).isoformat(), "os": platform.platform(),
        "cpu": next(s.split(':', 1)[1].strip() for s in Path('/proc/cpuinfo').read_text().splitlines() if s.startswith('model name')),
        "mem_total": next(s for s in Path('/proc/meminfo').read_text().splitlines() if s.startswith('MemTotal:')),
        "affinity": sorted(os.sched_getaffinity(0)), "uid": os.getuid(),
        "filesystem": command('findmnt', '-T', str(base), '-n', '-o', 'SOURCE,FSTYPE,OPTIONS'),
        "block_devices": command('lsblk', '-o', 'NAME,TYPE,ROTA,MODEL,FSTYPE,MOUNTPOINTS'),
        "rustc": command('rustc', '--version'), "cargo": command('cargo', '--version'),
        "revision": command('git', 'rev-parse', 'HEAD'), "dirty": command('git', 'status', '--short'),
        "build": 'cargo build --locked --release -p notes-core --example phase7_benchmark',
        "sha256": {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in
                   [binary, ROOT/'Cargo.lock', ROOT/'crates/notes-core/examples/phase7_benchmark.rs', Path(__file__).resolve()]},
        "storage_base": str(base), "conditions": 'Live workstation, not idle-isolated; no affinity pinning, cache dropping, thermal or frequency control. Other agents may compile/test. See continuous load samples. Source generation and manifest reads prewarm OS cache; DB-cold is not physical cold storage.',
        "budget_gate": 'not_evaluated_measure_first', "initial_load": load_sample(),
    }
    save(args.output / 'environment.json', environment)
    results = []
    for count in (1000, 50000, 'robustness'):
        prefix = args.output / str(count)
        stop = threading.Event()

        def monitor():
            with prefix.with_suffix('.load.jsonl').open('x') as stream:
                while not stop.is_set():
                    stream.write(json.dumps(load_sample()) + '\n')
                    stream.flush()
                    stop.wait(2)

        worker = threading.Thread(target=monitor)
        worker.start()
        start = time.monotonic()
        argv = [str(binary), str(count), str(base)]
        timed_out = False
        try:
            with prefix.with_suffix('.raw.jsonl').open('x') as stdout, prefix.with_suffix('.stderr.txt').open('x') as stderr:
                try:
                    proc = subprocess.run(argv, stdout=stdout, stderr=stderr, timeout=1800, check=False)
                    code = proc.returncode
                except subprocess.TimeoutExpired:
                    code, timed_out = None, True
        finally:
            stop.set()
            worker.join()
        records = [json.loads(s) for s in prefix.with_suffix('.raw.jsonl').read_text().splitlines()]
        result = {"corpus": count, "command": argv, "returncode": code, "timed_out": timed_out,
                  "wall_seconds": time.monotonic()-start, "metrics": summarize(records),
                  "budget_gate": 'not_evaluated_measure_first'}
        results.append(result)
        save(args.output / 'summary.json', results)
        print(json.dumps(result), flush=True)
        if code != 0:
            raise RuntimeError(f'{count} failed; partial raw evidence retained, no retry discarded')
        if count == 'robustness':
            cases = [r for r in records if r['kind'] == 'robustness']
            assert len(cases) == 21
            assert all(r['accepted'] == r['expected_accepted'] for r in cases)
        else:
            assert records[-1]['kind'] == 'complete' and records[-1]['notes'] == count
            counts = Counter(r['metric'] for r in records if r['kind'] == 'sample')
            assert counts == Counter({'db_cold_rebuild_ms': 3, 'warm_reconcile_ms': 5,
                                     'warm_process_open_status_ms': 5, 'search_session_acquire_ms': 1,
                                     'search_api_common_ms': 10,
                                     'search_selective_ms': 100, 'search_common_ms': 100,
                                     'search_unicode_ms': 100, 'search_absent_ms': 100,
                                     'watcher_start_ms': 1, 'watcher_convergence_ms': 20}), counts
    assert [r['corpus'] for r in results] == [1000, 50000, 'robustness']
    save(args.output / 'verified.json', {"complete": True, "sample_counts_checked": True,
                                      "budget_gate": 'not_evaluated_measure_first'})


if __name__ == '__main__':
    main()
