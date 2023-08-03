#!/bin/bash

export DYNEMOLWORKDIR=$(pwd)/$1
export DYNEMOLDIR=/scratch/luis/development/SN

source /opt/intel/oneapi/setvars.sh > /dev/null

cd $(pwd)/$1

$DYNEMOLDIR/dynemol
