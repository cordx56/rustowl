#!/bin/bash

cd "$(dirname $0)"
cd ../algo-tests

for i in {1..3}
do
    for j in {1..2}
    do
        echo "Check f${i} v${j}"
        RESULT="$(rustowl show f${i} v${j} 2> /dev/null)"
        ASSERT="$(cat f${i}_v${j}.txt)"
        if [ "$RESULT" != "$ASSERT" ]; then
            diff -u <(echo "$RESULT") <(echo "$ASSERT")
            exit 1
        fi
    done
done
