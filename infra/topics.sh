#!/usr/bin/env bash
set -euo pipefail

rpk topic create raw-events --partitions 12 -r 1
rpk topic create raw-events-dlq --partitions 1 -r 1
rpk topic create replay_events --partitions 6 -r 1
rpk topic create replay_events-dlq --partitions 1 -r 1
