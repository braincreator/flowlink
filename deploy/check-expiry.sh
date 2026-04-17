#!/bin/bash
# FlowLink — Daily subscription expiry checker
# Calls POST /api/billing/check-expiry to process expiring subscriptions
#
# INSTALL:
#   sudo cp check-expiry.sh /opt/flowlink/scripts/check-expiry.sh
#   sudo chmod +x /opt/flowlink/scripts/check-expiry.sh
#   sudo cp flowlink-expiry.service /etc/systemd/system/
#   sudo cp flowlink-expiry.timer /etc/systemd/system/
#   sudo systemctl daemon-reload
#   sudo systemctl enable --now flowlink-expiry.timer
#
# VERIFY:
#   sudo systemctl list-timers flowlink-expiry.timer
#   sudo journalctl -u flowlink-expiry.service

curl -sf -X POST http://localhost:8080/api/billing/check-expiry > /dev/null 2>&1
