# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.4.x   | :white_check_mark: |
| 0.3.x   | :white_check_mark: |
| 0.2.x   | :white_check_mark: |
| < 0.1   | :x: |

## Reporting a Vulnerability

We take the security of CHRONODESK seriously. If you believe you have found a security vulnerability, please report it to us as follows:

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report them via the GitHub Security Advisory:

:lock: [Report a vulnerability](https://github.com/mrmedani/chronodesk/security/advisories/new)

### What to include

- Type of vulnerability
- Full reproduction steps
- Affected versions
- Potential impact
- Any suggested fixes (if known)

### What to expect

- **Acknowledgment** within 48 hours
- **Initial assessment** within 5 business days
- **Regular updates** on progress (every 7 days minimum)
- **Coordinated disclosure** — we will work with you on timing

## Disclosure Policy

We follow a 90-day disclosure window. After a fix is released, we will publish a security advisory detailing the issue.

## Bug Bounties

At this time we do not offer a paid bug bounty program. We will publicly acknowledge your contribution in our security hall of fame (with your permission).

## Encryption

CHRONODESK uses **AES-256-GCM** via the `ring` + `aes-gcm` crates for encrypting data channel messages. Key exchange uses **ECDH (X25519)** with ephemeral keys per session, negotiated at connection start via a handshake message. All subsequent messages are encrypted transparently. Legacy unencrypted messages are also accepted for backward compatibility.
