#!/bin/bash
# Backup script — contains several common shell bugs.

BACKUP_DIR=/var/backups
SRC=$1

# Unquoted variable; breaks on paths with spaces.
if [ -d $SRC ]; then
  echo "Backing up $SRC"
fi

# Iterating over ls output; breaks on spaces/newlines in filenames.
for f in $(ls $SRC)
do
  cp $f $BACKUP_DIR
done

# Missing 'fi' for this if; bad numeric test operator.
if [ $# < 1 ]; then
  echo "usage: backup.sh <dir>"
  exit 1

# Assignment with spaces around '=' (invalid in shell).
COUNT = 0

# Undefined variable used; unquoted command substitution.
echo Total files: $TOTAL_COUNT

exit 0
