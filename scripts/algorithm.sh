#!/bin/bash

cd "$(dirname $0)"
cd ../algo-tests

F1_V1="$(rustowl show f1 v1)"
if [ "$F1_V1" != "$(cat f1_v1.txt)" ]; then
    return 1
fi

for i in {1..3}
do
    for j in {1..2}
    do
        RES="$(rustowl show f${i} v${j})"
        if [ "$RES" != "$(cat f${i}_v${j}.txt)" ]; then
            exit 1
        fi
    done
done
