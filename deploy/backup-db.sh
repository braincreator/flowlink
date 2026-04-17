#!/bin/bash
# FlowLink — PostgreSQL database backup
# Creates a gzipped dump and keeps the last 30 backups
#
# INSTALL:
#   sudo cp backup-db.sh /opt/flowlink/scripts/backup-db.sh
#   sudo chmod +x /opt/flowlink/scripts/backup-db.sh
#   sudo cp flowlink-backup.service /etc/systemd/system/
#   sudo cp flowlink-backup.timer /etc/systemd/system/
#   sudo systemctl daemon-reload
#   sudo systemctl enable --now flowlink-backup.timer
#
# MANUAL RUN:
#   sudo /opt/flowlink/scripts/backup-db.sh
#
# VERIFY:
#   sudo systemctl list-timers flowlink-backup.timer
#   sudo journalctl -u flowlink-backup.service
#   ls -la /opt/flowlink/backups/

set -euo pipefail

BACKUP_DIR="/opt/flowlink/backups"
mkdir -p "$BACKUP_DIR"

FILENAME="flowlink_$(date +%Y%m%d_%H%M%S).sql.gz"

PGPASSWORD="9dd438d17436912e1e0c89d33fb82182" \
  pg_dump -h 127.0.0.1 -p 5433 -U supabase_admin -d postgres \
  | gzip > "$BACKUP_DIR/$FILENAME"

# Keep last 30 backups
ls -t "$BACKUP_DIR"/flowlink_*.sql.gz 2>/dev/null | tail -n +31 | xargs -r rm -f

echo "Backup complete: $FILENAME"
