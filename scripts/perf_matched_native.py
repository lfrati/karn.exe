"""Run the typed resident matrix on materialized, validated matched streams.

This measures native typed execution, separate from JSON transport or external players.
The one engine process runs every matrix row, confined to one logical CPU of each of as many
distinct physical cores as its largest row has workers (perf_parallel.placement), so no
row's threads need share a core's SMT threads; the report records the topology and placement.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import time
from perf_parallel import ROOT, placement, read, topology


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--engine',type=Path,required=True)
    ap.add_argument('--config',type=Path,required=True)
    ap.add_argument('--prepared',type=Path,required=True)
    ap.add_argument('--engine-source',required=True,help='Revision used to build this artifact')
    ap.add_argument('--copies',type=int,default=128)
    ap.add_argument('--allow-shared-cores',action='store_true',
                    help='run more workers than physical cores on SMT siblings, with a warning')
    ap.add_argument('--output',type=Path,required=True)
    args=ap.parse_args();available=sorted(os.sched_getaffinity(0))
    if args.copies<1:ap.error('positive copies required')
    config=json.loads(args.config.read_text());config['copies']=args.copies
    if min(config['workers'])<1:ap.error('positive workers required')
    topo=topology(available);placed=placement(topo,max(config['workers']),args.allow_shared_cores)
    for warning in placed['warnings']:print('warning:',warning,flush=True)
    cpus=placed['cpus'];os.sched_setaffinity(0,cpus)
    config_path=args.output.with_suffix('.config.json');args.output.parent.mkdir(parents=True,exist_ok=True)
    config_path.write_text(json.dumps(config,indent=2)+'\n')
    report={'engine_source':args.engine_source,'revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
            'engine_sha256':hashlib.sha256(args.engine.read_bytes()).hexdigest(),
            'harness_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'prepared_report_sha256':hashlib.sha256(args.prepared.read_bytes()).hexdigest(),
            'config':config,'config_sha256':hashlib.sha256(config_path.read_bytes()).hexdigest(),
            'traces':[{ 'path':p,'sha256':hashlib.sha256(Path(p).read_bytes()).hexdigest()} for p in config['traces']],
            'topology':topo,'placement':placed,
            'budget':{'cpu_affinity':cpus,'available_affinity':available,'address_space_limit':resource.getrlimit(resource.RLIMIT_AS),
                'sampled_process_rss_check_kib':config['rss_budget_kib'],'physical_memory':read('/proc/meminfo'),
                'cpu_hardware':read('/proc/cpuinfo'),'cgroup_membership':read('/proc/self/cgroup')},
            'status':'running','scope':'Native typed replay capacity only; same validated stream, obsolete discovery diagnostics skipped; includes viewer snapshots/disclosures and clone/reset. No Python/JSON/transport in timed region.'}
    args.output.write_text(json.dumps(report,indent=2)+'\n')
    began=time.perf_counter()
    proc=subprocess.run([str(args.engine.resolve()),'--bench-native',str(config_path.resolve())],cwd=ROOT,capture_output=True,text=True)
    report['total_process_seconds']=time.perf_counter()-began
    if proc.returncode:
        report.update(status='failed',returncode=proc.returncode,error=proc.stderr)
    else:
        report.update(status='complete',result=json.loads(proc.stdout))
    args.output.write_text(json.dumps(report,indent=2)+'\n')
    if proc.returncode:raise SystemExit(proc.stderr)
    for row in report['result']['matrix']:print(row['workers'],round(row['median_transitions_s'],2),'native transitions/s')
    print('Report:',args.output)

if __name__=='__main__':main()
