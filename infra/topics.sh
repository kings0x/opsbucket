#!/usr/bin/env bash
set -euo pipefail

rpk topic create raw-events --partitions 12 -r 1
rpk topic create raw-events-dlq --partitions 1 -r 1
