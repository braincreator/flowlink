# Terms of Service — FlowLink

**Last Updated:** April 3, 2026  
**Effective Date:** April 3, 2026  
**Version:** 1.0

---

## 1. Acceptance of Terms

By installing, using, or accessing FlowLink ("the Software"), you agree to be bound by these Terms of Service ("Terms"). If you do not agree to these Terms, do not use the Software.

FlowLink is developed and operated by FlowMasters ("we", "us", "our").

---

## 2. Description of Service

FlowLink is an **open-source infrastructure routing tool** that:

- Routes commands from AI assistants to servers you own and control
- Provides a relay server for command routing
- Offers optional managed cloud hosting for the relay

**FlowLink does NOT:**
- Control, modify, or initiate commands
- Provide AI or LLM services
- Access, process, or store your data beyond routing
- Execute commands autonomously

---

## 3. User Responsibilities

**You are solely responsible for:**

- All commands executed on your servers via FlowLink
- The behavior of any AI assistant connected to FlowLink
- The security of your API tokens and credentials
- Ensuring you have the right to execute commands on all connected servers
- Verifying commands before execution, even when using auto-approval mode
- Maintaining backups of your data (FlowLink's backup feature is a convenience, not a guarantee)

---

## 4. Liability Limitations

### 4.1 FlowLink is a Routing Tool

FlowLink functions as a communication relay, similar to SSH, VPN, or remote desktop software. We do not:

- Inspect, modify, or approve commands before routing
- Guarantee the safety or correctness of any command
- Control the behavior of connected AI assistants

### 4.2 Maximum Liability

**Our total liability for any claim arising from the use of FlowLink shall not exceed the fees you paid to us in the 12 months preceding the claim.**

For users of the free (self-hosted) version, liability is limited to $0.

### 4.3 No Warranty

FlowLink is provided "AS IS" and "AS AVAILABLE" without warranties of any kind, either express or implied, including but not limited to:

- Merchantability
- Fitness for a particular purpose
- Non-infringement
- Uninterrupted or error-free operation

### 4.4 Indemnification

You agree to indemnify, defend, and hold harmless FlowMasters from any claims, damages, losses, or expenses (including legal fees) arising from:

- Your use of FlowLink
- Commands executed on your servers
- Data loss, system damage, or service interruption
- Third-party claims related to your use of the Software

---

## 5. Prohibited Uses

You may not use FlowLink to:

- Execute commands on servers you do not own or have authorization to access
- Distribute malware, ransomware, or any malicious software
- Circumvent security measures of third-party systems
- Violate any applicable laws or regulations
- Use the service for any purpose that is unlawful or harmful to others

---

## 6. Safety Features

FlowLink includes safety mechanisms to reduce risk, but **these do not guarantee safety**:

| Feature | Purpose | Limitation |
|---------|---------|------------|
| Sandbox | Block dangerous command patterns | Cannot cover all possible destructive commands |
| Read-only mode | Prevent write operations | Must be explicitly enabled by user |
| Approval system | Require human confirmation | Can be bypassed by setting mode to "auto" |
| Auto-backup | Create snapshots before destructive commands | Backup may fail; does not cover all data |
| Kill switch | Emergency stop | Requires manual activation or monitoring setup |
| Command blacklist | Block known dangerous patterns | Cannot anticipate all dangerous commands |
| Timeout | Limit command execution time | Does not prevent instant damage |

**You acknowledge that no safety feature is foolproof and that you remain responsible for all actions taken on your servers.**

---

## 7. Data & Privacy

- FlowLink does not collect, store, or process your command output or file contents
- Audit logs are stored locally on your servers
- The relay server routes messages but does not persist command content
- We do not sell, share, or access your data
- You can self-host FlowLink for complete data sovereignty

---

## 8. Cloud Service (Paid)

If you use FlowLink Cloud:

- Payment is processed through our payment provider
- We reserve the right to suspend service for non-payment
- No refunds for partial months
- We provide a 14-day free trial for new accounts
- Pricing may change with 30 days notice

---

## 9. Open Source License

The FlowLink agent and relay are released under the [MIT License](LICENSE). This Terms of Service applies to the **Cloud service and any hosted infrastructure** provided by FlowMasters.

---

## 10. Termination

- You may stop using FlowLink at any time
- We may terminate Cloud service for Terms violations with 7 days notice
- Upon termination, your data is deleted within 30 days

---

## 11. Changes to Terms

We may update these Terms with 30 days notice. Continued use constitutes acceptance.

---

## 12. Contact

For questions about these Terms:

- **Website:** https://flow-masters.ru
- **Telegram:** https://t.me/flowmasters_ai_sales_bot
- **Email:** legal@flow-masters.ru
- **GitHub:** https://github.com/braincreator/flowlink/issues

---

## 13. Governing Law

These Terms are governed by the laws of the Russian Federation. Any disputes shall be resolved in the courts of Moscow, Russia.

---

*This document does not constitute legal advice. For specific legal questions, consult a qualified attorney.*
