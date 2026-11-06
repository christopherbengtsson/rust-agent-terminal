#!/usr/bin/env python3
"""Read-only additional forensics for paced_session_measurement17 artifacts.
Public History assertions remain in the real dashboard/supervisor test.
Streams transcript chunks only through the SQLite committed prefix; never loads
whole transcripts/record lists. Run after the artifact directory is quiescent.
"""
import collections
import hashlib
import json
import pathlib
import re
import sqlite3
import statistics
import sys

root = pathlib.Path(sys.argv[1]).resolve()
events = [json.loads(line) for line in (root / 'observations.jsonl').open()]

def distribution(values):
    ordered = sorted(values)
    if not ordered:
        return {'n': 0}
    return {'n': len(ordered), **{f'p{p}': ordered[min(len(ordered)-1, (len(ordered)*p+99)//100-1)] for p in (50, 90, 95, 99)}, 'min': ordered[0], 'max': ordered[-1]}

runs = {}
for line in (root / 'proof-emitted17.jsonl').open():
    value = json.loads(line)
    if value.get('kind') == 'complete':
        runs[value['run']] = value
inputs = {v['nonce']: v for v in map(json.loads, (root/'proof-input17.jsonl').open())}
report = {'manifest': events[0], 'runs': runs, 'latency_us': {}, 'rss_KiB': {}, 'raw_continuity': []}
for kind in ('navigation', 'attach'):
    for mode in sorted({e['mode'] for e in events if e['kind'] == kind}):
        samples = [e for e in events if e['kind'] == kind and e['mode'] == mode]
        values = [e['us'] for e in samples if e.get('us') is not None]
        valid_active = []
        for e in samples:
            run = runs.get(e.get('run'))
            if run and e.get('visible_ns') is not None:
                e['active_entire_interval'] = run['start_ns'] <= e['injected_ns'] <= e['visible_ns'] < run['finish_ns']
                if e['active_entire_interval']:
                    valid_active.append(e['us'])
            nonce = e.get('nonce')
            if nonce in inputs:
                e['fixture_received_ns'] = inputs[nonce]['received_ns']
                e['input_to_fixture_us'] = (inputs[nonce]['received_ns']-e['injected_ns'])/1000
        report['latency_us'][kind+'/'+mode] = {'all': distribution(values), 'entirely_active': distribution(valid_active), 'censored': sum(e.get('visible_ns') is None for e in samples)}
report['latency_samples'] = [e for e in events if e['kind'] in ('navigation','attach')]
for run_id, run in runs.items():
    duration = (run['finish_ns']-run['start_ns'])/1e9
    run['achieved_Bps'] = run['bytes']/duration
    pids = collections.defaultdict(list)
    for e in events:
        if e['kind'] == 'rss' and run['start_ns'] <= e['sample_ns'] < run['finish_ns']:
            for line in e['ps'].splitlines():
                columns = line.split(None, 4)
                if len(columns) == 5:
                    pids[columns[0]].append((e['sample_ns'], int(columns[2]), columns[4]))
    for pid, samples in pids.items():
        # Equal first/middle/last windows, excluding first five seconds.
        steady = [s for s in samples if s[0] >= run['start_ns']+5_000_000_000]
        windows = [steady[:10],steady[max(0,len(steady)//2-5):len(steady)//2+5],steady[-10:]]
        report['rss_KiB'][f'run{run_id}/pid{pid}'] = {'command': samples[0][2], 'samples':len(samples), 'sampled_peak':max(s[1] for s in samples),'steady':distribution([s[1] for s in steady]),'first_middle_last_median':[statistics.median([s[1] for s in w]) if w else None for w in windows]}
commits = [e['commit'] for e in events if e['kind']=='commit']
assert [c['sequence'] for c in commits] == list(range(1,len(commits)+1)), 'missing commit observations'
report['commits'] = {'count':len(commits),'interval_us': distribution([b['completed_micros']-a['completed_micros'] for a,b in zip(commits,commits[1:])]),'oldest_accepted_us':distribution([c['oldest_accepted_micros'] for c in commits]),'sync_commit_us':distribution([c['sync_commit_micros'] for c in commits])}
# Classify intervals only when both endpoints fall within a healthy producer run.
# Worker-relative origin differs from the common kernel clock: use per-run prefix
# ranges from the raw scanner below to classify completion intervals by prefix.
marker = b'\x1b[2;1H\x1b[2Kpace17 run='
pattern = re.compile(rb'^\x1b\[2;1H\x1b\[2Kpace17 run=(\d+) seq=(\d{8})\x1b\[3;1H\x1b\[32m\xce\xbb\xe7\x95\x8c\x1b\[0mX+$')

def inspect(directory):
    db=sqlite3.connect(f'file:{directory / "history.sqlite3"}?mode=ro',uri=True)
    output=[]
    for session_id,identity,durable in db.execute('select id,identity,durable from sessions order by rowid'):
        digest=hashlib.sha256(); counts=collections.Counter(); ranges={}; carry=b''; remaining=durable; raw_offset=0
        chunk=0
        while remaining:
            path=directory/f'{session_id}-{chunk:08}.transcript'
            with path.open('rb') as stream:
                take=min(65536,remaining)
                data=stream.read(take)
                assert len(data)==take, f'truncated committed chunk: {path}'
            digest.update(data); remaining-=take; chunk+=1; carry+=data
            # Prefix scanner keeps at most one incomplete 1024-byte record plus
            # the next 64KiB input block. Unknown bytes remain in the hash.
            while True:
                position=carry.find(marker)
                if position<0:
                    trim=max(0,len(carry)-len(marker)); carry=carry[trim:]; raw_offset+=trim; break
                if position:
                    carry=carry[position:];raw_offset+=position
                if len(carry)<1024: break
                record=carry[:1024]; match=pattern.fullmatch(record)
                assert match, f'malformed producer record at {raw_offset}: {record[:100]!r}'
                run,sequence=map(int,match.groups())
                assert sequence==counts[run], f'gap/duplicate run {run}: {sequence}, expected {counts[run]}'
                counts[run]+=1
                ranges.setdefault(run,[raw_offset,0])[1]=raw_offset+1024
                carry=carry[1024:]; raw_offset+=1024
        output.append({'identity':identity,'session':session_id,'durable':durable,'sha256':digest.hexdigest(),'complete_records':dict(counts),'record_prefix_ranges':ranges,'trailing_incomplete_bytes':len(carry) if carry.startswith(marker) else 0})
    db.close();return output

report['raw_continuity']=inspect(root)
for session in report['raw_continuity']:
    for run, ranges in session['record_prefix_ranges'].items():
        intervals=[]; ages=[]; sync=[]
        for a,b in zip(commits,commits[1:]):
            if ranges[0]<=a['prefix']<=b['prefix']<=ranges[1]:
                intervals.append(b['completed_micros']-a['completed_micros']);ages.append(b['oldest_accepted_micros']);sync.append(b['sync_commit_micros'])
        report['commits'][f'run{run}']={'interval_us':distribution(intervals),'oldest_accepted_us':distribution(ages),'sync_commit_us':distribution(sync)}
    for run, count in session['complete_records'].items():
        if run in runs:
            assert count==runs[run]['records'],f'completed run {run}: emitted/captured record mismatch'
crash=root/'crash-before-restore'
if crash.exists():
    report['crash_raw_continuity']=inspect(crash)
    observation=next(e for e in events if e['kind']=='crash')
    report['crash']=observation
    recovered=report['crash_raw_continuity'][-1]['durable']
    report['crash']['accepted_minus_recovered']=observation['acknowledged']['accepted_bytes']-recovered
    assert recovered>=observation['acknowledged']['durable_bytes']
report['disk_samples']=[e for e in events if e['kind']=='disk']
report['sampler_us']=distribution([e['sampler_us'] for e in events if e['kind']=='rss'])
report['phases']=[e for e in events if e['kind'] in ('phase','retry','stop','finished')]
(root/'analysis.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({key:report[key] for key in ('latency_us','commits','sampler_us','raw_continuity')},indent=2))
