#!/bin/bash

export PATH=/home/elrond/Github/wabt/build:$PATH

./wat-build.sh dex/router
./wat-build.sh dex/pair
./wat-build.sh dex/farm

./wat-build.sh locked-asset/distribution
./wat-build.sh locked-asset/factory
./wat-build.sh locked-asset/proxy-dex
