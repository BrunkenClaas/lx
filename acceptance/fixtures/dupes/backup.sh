#!/bin/bash
# Database dump — backs up the postgres DB to /mnt/backups
set -e
pg_dump mydb | gzip > /mnt/backups/mydb-$(date +%Y%m%d).sql.gz
echo "Backup complete"
