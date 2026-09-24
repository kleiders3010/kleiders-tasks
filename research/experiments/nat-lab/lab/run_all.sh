#!/bin/bash
# Full E10 matrix (lab must be up: all_up.sh; latency: lab.sh delay 20).
cd "$(dirname "$0")"
R=${1:-../results}
python3 run_matrix.py baseline        --out $R
python3 run_matrix.py relay           --out $R --reps 3
python3 run_matrix.py relay_idonly    --out $R
python3 run_matrix.py relay_cross     --out $R
python3 run_matrix.py relay_noportmap --out $R
python3 run_matrix.py direct_one      --out $R
python3 run_matrix.py direct_simul    --out $R --reps 3
python3 run_matrix.py direct_stun     --out $R --reps 3
echo ALLDONE
