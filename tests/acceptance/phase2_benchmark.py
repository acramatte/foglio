#!/usr/bin/env python3
"""Run the built release example on fixed 1k/50k synthetic corpora; save real results."""
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]

def command(*args):
    return subprocess.check_output(args, text=True).strip()

def main():
    output = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / 'docs/phase2-baseline.json'
    report = {'environment': {'os': platform.platform(), 'cpu': next(line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), 'memory': next(line for line in Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemTotal:')), 'filesystem': command('findmnt','-T',tempfile.gettempdir(),'-n','-o','SOURCE,FSTYPE,OPTIONS'), 'storage': command('lsblk','-d','-o','NAME,ROTA,MODEL'), 'rustc': command('rustc','--version'), 'build': 'release', 'available_cpus': len(os.sched_getaffinity(0))}, 'corpora': []}
    for count in (1000,50000):
        result = json.loads(command(str(ROOT/'target/release/examples/phase2_benchmark'),str(count)))
        for field in ('cold_rebuild_ms','warm_reconcile_ms','search_ms'):
            values = sorted(result[field])
            result[field+'_p50'] = values[math.ceil(len(values)*.50)-1]
            result[field+'_p95'] = values[math.ceil(len(values)*.95)-1]
        report['corpora'].append(result)
        output.parent.mkdir(parents=True,exist_ok=True)
        output.write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps({k:v for k,v in result.items() if not isinstance(v,list)}),flush=True)
    assert [r['notes'] for r in report['corpora']] == [1000,50000]

if __name__ == '__main__':
    main()
