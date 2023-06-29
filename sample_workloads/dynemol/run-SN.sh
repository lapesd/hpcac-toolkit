#!/bin/bash

export DYNEMOLWORKDIR=$(pwd)
export DYNEMOLDIR=/scratch/luis/development/SN

source /opt/intel/oneapi/setvars.sh > /dev/null

$DYNEMOLDIR/dynemol
