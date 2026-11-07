#!/usr/bin/env python3
"""Summarize only recorded six_stream_coordination_measurement19 evidence."""
import collections
import json
import math
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
rows = [json.loads(line) for line in (root / 'observations.jsonl').read_text().splitlines()]

def distribution(values):
    values = sorted(values)
    return {'n': len(values), **({k: values[min(len(values) - 1, math.ceil(p * len(values)) - 1)] for k, p in [('p50_us', .5), ('p95_us', .95), ('p99_us', .99), ('worst_us', 1)]} if values else {})}

summary = {'manifest': next(row for row in rows if row['kind'] == 'manifest')}
summary['latency'] = {}
for kind in ['navigation', 'attach', 'search', 'diff', 'stop']:
    phases = collections.defaultdict(list)
    for row in rows:
        if row['kind'] == kind:
            phases[row.get('phase', 'healthy-search-diff')].append(row)
    summary['latency'][kind] = {phase: {**distribution([row['us'] for row in samples if isinstance(row.get('us'), (int, float))]), 'censored': sum(row.get('visible_ns') is None for row in samples if kind == 'attach')} for phase, samples in phases.items()}
summary['streams'] = []
for index in range(6):
    final = json.loads((root / f'final-stream-{index}.json').read_text())
    commits = next(row['commits'] for row in rows if row['kind'] == 'all-commits' and row['index'] == index)
    snapshots = [row for row in rows if row['kind'] == 'stream-snapshot' and row['index'] == index]
    pre_fault = max(snapshot['durable_commits'][-1]['sequence'] for snapshot in snapshots if snapshot['phase'] == 'healthy-end')
    healthy = [commit for commit in commits if index != 0 or commit['sequence'] <= pre_fault]
    summary['streams'].append({'index': index, 'identity': next(row for row in rows if row['kind'] == 'stream-identity' and row['index'] == index), 'accepted': final['accepted_bytes'], 'written': final['written_bytes'], 'durable': final['durable_bytes'], 'pending': final['capture_pending_bytes'], 'capture_high_water': final['capture_high_water'], 'backpressure_count': final['capture_backpressure_count'], 'subscriber_frame_high_water': final['subscriber_frame_high_water'], 'commits': len(commits), 'max_healthy_accepted_age_us': max(commit['oldest_accepted_micros'] for commit in healthy), 'max_healthy_completion_gap_us': max(b['completed_micros'] - a['completed_micros'] for a, b in zip(healthy, healthy[1:])), 'retained_raw': next(row for row in rows if row['kind'] == 'retained-raw' and row['index'] == index)})
rss = collections.defaultdict(list)
disks = []
for row in rows:
    if row['kind'] != 'resources':
        continue
    for line in row['ps'].splitlines():
        fields = line.strip().split(maxsplit=4)
        if len(fields) >= 4:
            rss[fields[0]].append(int(fields[2]))
    files = row['files']
    disks.append({'ns': row['sample_ns'], 'transcript_bytes': sum(file['bytes'] for file in files if file['name'].endswith('.transcript')), 'transcript_allocated_bytes': sum(file['allocated_bytes'] for file in files if file['name'].endswith('.transcript')), 'sqlite_bytes': sum(file['bytes'] for file in files if file['name'].startswith('history.sqlite3'))})
summary['rss_kib_by_pid'] = {pid: {'n': len(values), 'first': values[0], 'last': values[-1], 'peak': max(values)} for pid, values in rss.items()}
summary['disk_samples'] = disks
summary['windows'] = next(row for row in rows if row['kind'] == 'windows')
summary['result'] = next(row for row in rows if row['kind'] == 'result')
summary['limitations'] = ['Fixture coding streams, not Claude/Codex throughput or semantic status.', 'macOS portable-pty/vt100 terminal cells, not graphical pixel timing.', 'One stream uses an injected ENOSPC write fault, not actual volume exhaustion.', 'Faulted visible native output can remain blocked: censored attachment is not a healthy attach pass.', 'RSS and disk are samples; no allocator maximum or power-loss guarantee follows.', 'completed_micros is relative to the storage worker origin; oldest_accepted_micros already stores age.']
(root / 'analysis.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2))
