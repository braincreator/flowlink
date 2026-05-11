# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in FlowLink, please report it responsibly:

- **Email:** flowlink@flow-masters.ru
- **Response SLA:** Acknowledgment within 24h, assessment within 72h

Please do not file public GitHub issues for security vulnerabilities.

## Code Audit Program

FlowLink is proprietary software with a **source-available audit program**.

### Who can request a code audit

- Enterprise customers evaluating FlowLink
- Security researchers and auditors
- Partners integrating FlowLink into their stack
- Compliance teams requiring source code review (SOC 2, EU AI Act, ФЗ-152)

### How to request

1. Email [flowlink@flow-masters.ru](mailto:flowlink@flow-masters.ru?subject=Code%20Audit%20Request) with your organization details and audit scope
2. Sign a mutual NDA
3. Receive time-limited access to the source repository
4. Submit findings through our responsible disclosure process

### What you'll see

Full source code of all components:
- Gateway (auth, billing, API)
- Relay (MCP proxy, policy engine, risk scoring)
- Shield (eBPF programs, runtime monitoring)
- Dashboard (Next.js frontend)

## Security Architecture

FlowLink implements multiple security layers:

- **eBPF Shield:** 11 kernel-level BPF programs monitoring agent processes
- **E2EE Relay:** End-to-end encrypted agent communication (keys stay on your infra)
- **Policy Engine:** Configurable access control for all agent actions
- **Audit Trail:** Tamper-proof logging of every interaction
- **Approval Workflows:** Human-in-the-loop for high-risk operations
