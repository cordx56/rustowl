#!/bin/bash

cd "$(dirname $0)"
cd ../algo-tests

for i in {1..3}
do
    for j in {1..2}
    do
        echo "Check f${i} v${j}"
        RES="$(rustowl show f${i} v${j} 2> /dev/null)"
        if [ "$RES" != "$(cat f${i}_v${j}.txt)" ]; then
            exit 1
        fi
    done
done
