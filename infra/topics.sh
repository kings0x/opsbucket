#!/usr/bin/env bash
set -euo pipefail

rpk topic create raw-events --partitions 3 --replication-factor 1
rpk topic create raw-events-dlq --partitions 1 --replication-factor 1
