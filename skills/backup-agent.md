---
name: Backup Agent
version: 0.1.0
description: Automated backups for files, databases (PostgreSQL, MySQL, MongoDB) with rotation and S3 sync.
icon: 💾
commands:
  - name: backup_create
    description: Create a new backup
    run: |
      TYPE={type} && \
      NAME={name} && \
      TIMESTAMP=$(date +%Y%m%d_%H%M%S) && \
      BACKUP_DIR=/backups && \
      mkdir -p $BACKUP_DIR && \
      case $TYPE in
        files)
          tar -czf $BACKUP_DIR/${NAME}_${TIMESTAMP}.tar.gz -C {source} . && echo "Files backup created: ${NAME}_${TIMESTAMP}.tar.gz"
          ;;
        postgres)
          pg_dump -h {host} -U {user} {database} | gzip > $BACKUP_DIR/${NAME}_${TIMESTAMP}.sql.gz && echo "PostgreSQL backup created"
          ;;
        mysql)
          mysqldump -h {host} -u {user} -p{password} {database} | gzip > $BACKUP_DIR/${NAME}_${TIMESTAMP}.sql.gz && echo "MySQL backup created"
          ;;
        mongodb)
          mongodump --host {host} --db {database} --archive | gzip > $BACKUP_DIR/${NAME}_${TIMESTAMP}.archive.gz && echo "MongoDB backup created"
          ;;
      esac
    timeout: 300
    args:
      - name: type
        required: true
        description: Backup type (files, postgres, mysql, mongodb)
      - name: name
        required: true
        description: Backup name prefix
      - name: source
        required: false
        description: Source directory (for files backup)
      - name: host
        required: false
        description: Database host
        default: "localhost"
      - name: user
        required: false
        description: Database user
      - name: password
        required: false
        description: Database password
      - name: database
        required: false
        description: Database name
  - name: backup_list
    description: List all backups
    run: ls -lh /backups/ 2>/dev/null | tail -20 || echo "No backups found"
    timeout: 10
  - name: backup_restore
    description: Restore from backup
    run: |
      FILE={file} && \
      TYPE={type} && \
      case $TYPE in
        files)
          tar -xzf /backups/$FILE -C {destination} && echo "Files restored to {destination}"
          ;;
        postgres)
          gunzip -c /backups/$FILE | psql -h {host} -U {user} {database} && echo "PostgreSQL restored"
          ;;
        mysql)
          gunzip -c /backups/$FILE | mysql -h {host} -u {user} -p{password} {database} && echo "MySQL restored"
          ;;
        mongodb)
          gunzip -c /backups/$FILE | mongorestore --host {host} --db {database} --archive && echo "MongoDB restored"
          ;;
      esac
    timeout: 300
    args:
      - name: file
        required: true
        description: Backup filename
      - name: type
        required: true
        description: Backup type (files, postgres, mysql, mongodb)
      - name: destination
        required: false
        description: Restore destination (for files)
      - name: host
        required: false
        default: "localhost"
      - name: user
        required: false
      - name: password
        required: false
      - name: database
        required: false
  - name: backup_rotate
    description: Delete old backups, keep last N
    run: |
      KEEP={keep} && \
      cd /backups && \
      ls -t | tail -n +$(($KEEP + 1)) | xargs -r rm -f && \
      echo "Rotation complete. Kept last $KEEP backups." && \
      ls -lh /backups/ | wc -l && echo "backups remaining"
    timeout: 30
    args:
      - name: keep
        required: false
        description: Number of backups to keep
        default: "10"
  - name: backup_sync_s3
    description: Sync backups to S3-compatible storage
    run: |
      aws s3 sync /backups/ s3://{bucket}/{prefix}/ --endpoint-url={endpoint} --storage-class {storage_class} && \
      echo "Synced to S3: s3://{bucket}/{prefix}/"
    timeout: 120
    args:
      - name: bucket
        required: true
        description: S3 bucket name
      - name: prefix
        required: false
        description: S3 path prefix
        default: "backups"
      - name: endpoint
        required: false
        description: S3 endpoint URL (for S3-compatible storage)
      - name: storage_class
        required: false
        description: Storage class (STANDARD, GLACIER)
        default: "STANDARD"
  - name: backup_schedule
    description: Show backup schedule (cron jobs)
    run: crontab -l 2>/dev/null | grep backup || echo "No backup schedules found"
    timeout: 5
backup_types:
  - name: files
    description: tar.gz archives of directories
    extension: .tar.gz
  - name: postgres
    description: PostgreSQL database dumps
    extension: .sql.gz
  - name: mysql
    description: MySQL database dumps
    extension: .sql.gz
  - name: mongodb
    description: MongoDB database archives
    extension: .archive.gz
default_rotation:
  keep_count: 10
  schedule: "0 2 * * *"  # Daily at 2 AM
---

# Backup Agent

Comprehensive backup solution for files and databases with rotation and cloud sync.

## Supported Backup Types

| Type | Tool | File Extension |
|------|------|----------------|
| Files | tar.gz | .tar.gz |
| PostgreSQL | pg_dump | .sql.gz |
| MySQL | mysqldump | .sql.gz |
| MongoDB | mongodump | .archive.gz |

## Usage Examples

### Create Backups

```bash
# Backup files
backup_create type=files name=myapp source=/var/www/myapp

# Backup PostgreSQL
backup_create type=postgres name=mydb host=localhost user=postgres database=myapp

# Backup MySQL
backup_create type=mysql name=mydb host=localhost user=root password=secret database=myapp

# Backup MongoDB
backup_create type=mongodb name=mydb host=localhost database=myapp
```

### Manage Backups

```bash
# List all backups
backup_list

# Rotate old backups (keep last 10)
backup_rotate keep=10

# Restore from backup
backup_restore file=myapp_20240115_020000.tar.gz type=files destination=/var/www/myapp
```

### Cloud Sync

```bash
# Sync to AWS S3
backup_sync_s3 bucket=my-backups prefix=server1

# Sync to S3-compatible storage (e.g., MinIO, Wasabi)
backup_sync_s3 bucket=my-backups endpoint=https://s3.wasabisys.com
```

## Backup Schedule

Default schedule runs daily at 2 AM. View current schedule:
```bash
backup_schedule
```

To add a new scheduled backup, add to crontab:
```bash
0 2 * * * /usr/local/bin/flowlink backup_create type=postgres name=mydb
```

## Rotation Policy

By default, keeps last 10 backups. Customize with:
```bash
backup_rotate keep=20
```

## Storage Locations

- **Local**: `/backups/` directory
- **S3**: Configurable bucket and prefix
- **S3-Compatible**: Any S3-compatible storage (MinIO, Wasabi, Backblaze)

## Best Practices

1. **3-2-1 Rule**: 3 copies, 2 different media, 1 offsite
2. **Test restores**: Regularly verify backups work
3. **Encrypt sensitive data**: Use GPG for encryption
4. **Monitor storage**: Alert when backup storage > 80%

## Environment Variables

```bash
# Database credentials
DB_HOST=localhost
DB_USER=postgres
DB_PASS=secret

# S3 credentials
AWS_ACCESS_KEY_ID=your_key
AWS_SECRET_ACCESS_KEY=your_secret
S3_ENDPOINT=https://s3.amazonaws.com
```
