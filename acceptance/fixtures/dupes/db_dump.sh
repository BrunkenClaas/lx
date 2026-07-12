#!/bin/bash
# Postgres backup — dumps mydb and compresses to backup storage
set -e
pg_dump mydb | gzip > /mnt/backups/mydb-$(date +%Y%m%d).sql.gz
echo "Done"
