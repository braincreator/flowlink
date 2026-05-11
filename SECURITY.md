# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in FlowLink, please report it responsibly:

- **Email:** flowlink@flow-masters.ru
- **PGP Key:** Available on request

Please do not file public GitHub issues for security vulnerabilities.

## Response Time

- **Acknowledgment:** Within 24 hours
- **Initial Assessment:** Within 72 hours
- **Fix Timeline:** Depends on severity (Critical: 48h, High: 7 days, Medium: 30 days)

## Security Architecture

FlowLink implements multiple security layers:

- **eBPF Shield:** Kernel-level monitoring of agent processes
- **E2EE Relay:** End-to-end encrypted agent communication
- **Policy Engine:** Configurable access control for all agent actions
- **Audit Trail:** Tamper-proof logging of every interaction
- **Approval Workflows:** Human-in-the-loop for high-risk operations
