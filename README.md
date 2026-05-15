<div align="center">

# 🛡️ FlowLink

**Governance & Risk Control for Autonomous AI Systems — enterprise-grade AI agent security and MCP governance layer.**

[![Latest Release](https://img.shields.io/github/v/release/braincreator/flowlink?label=latest&color=blue)](https://github.com/braincreator/flowlink/releases/latest)
[![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20macos-informational)](https://github.com/braincreator/flowlink/releases)
[![License](https://img.shields.io/badge/license-proprietary-red)]()
[![Code Audit](https://img.shields.io/badge/code_audit-available_NDA-green)](mailto:flowlink@flow-masters.ru)

*Policy enforcement · Risk scoring · Audit trails · MCP-native governance*

**MCP is becoming the standard protocol for AI agents. FlowLink is the governance layer they need.**

[Download Latest Release →](https://github.com/braincreator/flowlink/releases/latest)

</div>

---

## Why FlowLink?

When AI agents connect to your infrastructure via MCP, they get powerful access — file systems, databases, APIs, shell commands. **Without governance, a single misaligned agent can cause catastrophic damage.**

FlowLink sits between your MCP agents and your infrastructure, enforcing policies, scoring risks, and maintaining a complete audit trail.

**Think of it as a firewall for autonomous AI systems.**

## What It Does

- 🔒 **Policy Engine** — Define what agents can and cannot do: file access, network calls, command execution
- 🛡️ **Risk Scoring** — Every agent action scored in real-time with configurable thresholds
- 📋 **Audit Trail** — Complete, tamper-proof log of every agent interaction for compliance
- ✅ **Approval Workflows** — High-risk actions require human approval before execution
- 🔐 **E2EE Relay** — Encrypted communication channel between agents and your infrastructure
- 🏗️ **MCP-Native** — Built specifically for the Model Context Protocol — not a generic security tool

## Architecture

```
┌──────────────┐     ┌──────────────────┐     ┌──────────────┐
│  AI Agents   │────▶│    FlowLink      │────▶│  Your Infra  │
│  (MCP)       │     │  ┌────────────┐  │     │  (APIs, DBs, │
│              │     │  │  Policy    │  │     │  Files, SSH) │
│  Claude      │     │  │  Engine    │  │     │              │
│  GPT         │     │  ├────────────┤  │     │              │
│  OpenSource  │     │  │  Risk      │  │     │              │
│              │     │  │  Scorer    │  │     │              │
│              │     │  ├────────────┤  │     │              │
│              │     │  │  Audit     │  │     │              │
│              │     │  │  Logger    │  │     │              │
│              │     │  └────────────┘  │     │              │
└──────────────┘     └──────────────────┘     └──────────────┘
```

## Quick Start

### 1. Download

```bash
# Linux (x86_64)
curl -sL https://github.com/braincreator/flowlink/releases/latest/download/flowlink-linux-amd64.tar.gz | tar xz

# macOS (Apple Silicon)
curl -sL https://github.com/braincreator/flowlink/releases/latest/download/flowlink-darwin-arm64.tar.gz | tar xz
```

### 2. Configure

```bash
cat > policy.toml <<EOF
[default]
max_file_size = "10MB"
allowed_commands = ["ls", "cat", "grep", "git"]
network = "restricted"

[risk]
threshold = 0.7  # Actions above 70% risk score require approval
EOF
```

### 3. Run

```bash
./flowlink --config policy.toml
```

## Security & Trust

FlowLink handles your most sensitive infrastructure access. We take trust seriously:

- **Code Audit Available** — Source code is available for security review under NDA for enterprise customers, partners, and auditors. [Request access →](mailto:flowlink@flow-masters.ru?subject=Code%20Audit%20Request)
- **eBPF Shield** — Kernel-level monitoring with 11 BPF programs for runtime protection
- **E2EE Relay** — End-to-end encrypted agent communication, keys never leave your infrastructure
- **SOC 2 / EU AI Act Ready** — Audit trails and compliance reporting built in

*We believe security software should be verifiable, not just trusted. Reach out for a code audit.*

## Why This Matters

**EU AI Act** enforcement is coming. Organizations deploying AI agents will need:
- Audit trails for every autonomous decision
- Risk assessments for AI-powered actions
- Human oversight mechanisms for high-risk operations

FlowLink provides all three, purpose-built for the MCP ecosystem.

## Roadmap

- [x] Policy engine with file/command/network rules
- [x] Real-time risk scoring
- [x] E2EE relay for agent communication
- [x] eBPF-based Shield (11 BPF programs)
- [x] Approval workflows
- [x] Multi-region compliance (EU + RU)
- [ ] Red Team module (automated adversarial testing)
- [ ] Ops AI (intelligent incident response)
- [ ] MCP Governance RFC (open standard proposal)
- [ ] Zero-Trust agent authentication

## Pricing

| Plan | Agents | Price |
|------|--------|-------|
| **Free** | 1 | $0/mo |
| **Starter** | 5 | $99/mo |
| **Team** | 25 | $299/mo |
| **Business** | 100 | $499/mo |
| **Enterprise** | Unlimited | Custom |

Start free — upgrade when you need more agents. [Sign up →](https://flowlink.flow-masters.ru)

## Links

- 🌐 **Website:** [flowlink.flow-masters.ru](https://flowlink.flow-masters.ru)
- 📥 **Releases:** [Latest binaries](https://github.com/braincreator/flowlink/releases)
- 🔒 **Code Audit:** [Request under NDA](mailto:flowlink@flow-masters.ru?subject=Code%20Audit%20Request)
- 📧 **Contact:** [flowlink@flow-masters.ru](mailto:flowlink@flow-masters.ru)

## Learn More

- 📖 **[Documentation](https://flowlink.flow-masters.ru/docs)** — Full API reference, guides, and tutorials
- 🎮 **[Playground](https://flowlink.flow-masters.ru/playground)** — Interactive demo and sandbox
- ✨ **[Features](https://flowlink.flow-masters.ru/features)** — Detailed feature breakdown
- 💲 **[Pricing](https://flowlink.flow-masters.ru/pricing)** — Plans and billing details

## Comparisons

- 📊 **[FlowLink vs ToolHive](https://flowlink.flow-masters.ru/docs/comparison)** — Security, governance, and feature comparison

## License

Proprietary software. Binary releases provided for evaluation and production use. Source code available for security audit under NDA — [contact us](mailto:flowlink@flow-masters.ru?subject=Code%20Audit%20Request).

---

<div align="center">

**Built with Rust · Secured with eBPF · Governed by Policy**

*If your AI agents have access to your infrastructure, you need FlowLink.*

</div>
