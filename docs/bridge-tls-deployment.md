# Bridge TLS Deployment Guide

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
Security comes before convenience.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

---

## Mandatory TLS Requirement

The SPEX bridge server (`spex-bridge`) **must not** be exposed directly to external clients
over plain HTTP in any production deployment.

`docs/security.md` states: "Use TLS/HTTPS for all bridge and external API traffic."

This is not optional. TLS protects metadata and transport integrity. It does not replace
protocol-level signature and context checks, but is required in addition to them.

## Canonical Deployment Model: Reverse Proxy

The canonical production deployment model is **reverse-proxy TLS termination**.
The bridge binary itself listens on a local plaintext port (default `0.0.0.0:3000`).
A TLS-terminating reverse proxy sits in front and handles all external HTTPS connections.

```
Client
  |
  | HTTPS (TLS 1.2+ enforced)
  v
[Reverse Proxy: nginx / Caddy / HAProxy]
  |
  | HTTP (loopback / internal only)
  v
[spex-bridge :3000]
```

This separation means:
- The bridge binary does not need to manage certificates or TLS state.
- Certificate rotation happens at the proxy layer without restarting the bridge.
- The bridge socket must be bound only to localhost (`127.0.0.1:3000`) or an internal network
  interface, never exposed directly to the public internet.

---

## Nginx Configuration Example

```nginx
server {
    listen 443 ssl;
    server_name bridge.example.com;

    ssl_certificate     /etc/ssl/certs/bridge.crt;
    ssl_certificate_key /etc/ssl/private/bridge.key;

    # Enforce TLS 1.2+ only; no SSLv3 or TLS 1.0/1.1.
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    location / {
        proxy_pass         http://127.0.0.1:3000;
        proxy_set_header   Host $host;
        proxy_set_header   X-Real-IP $remote_addr;
        proxy_set_header   X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header   X-Forwarded-Proto $scheme;
    }
}

# Redirect plain HTTP to HTTPS.
server {
    listen 80;
    server_name bridge.example.com;
    return 301 https://$host$request_uri;
}
```

---

## Caddy Configuration Example

```caddy
bridge.example.com {
    reverse_proxy 127.0.0.1:3000
    # Caddy manages TLS certificates automatically via Let's Encrypt.
}
```

---

## Bridge Binding Configuration

To ensure the bridge cannot be reached over plain HTTP from outside the host, bind it to
loopback only. Set the `SPEX_BRIDGE_ADDR` environment variable (or modify the binary) to:

```bash
SPEX_BRIDGE_ADDR=127.0.0.1:3000 spex-bridge
```

If deploying in a containerized environment, the bridge container should not expose port 3000
directly to the host network. Use an internal Docker network and route traffic through the
proxy container.

---

## TLS Validation Checklist

Run this checklist before declaring a deployment production-ready.

### 1. Certificate validity

```bash
# Confirm the certificate is valid and not expired.
openssl s_client -connect bridge.example.com:443 -servername bridge.example.com \
  </dev/null 2>/dev/null | openssl x509 -noout -dates
```

Expected: `notAfter` is in the future.

### 2. TLS protocol enforcement

```bash
# Confirm TLS 1.0 and 1.1 are rejected.
openssl s_client -connect bridge.example.com:443 -tls1 </dev/null 2>&1 | grep -i "handshake failure"
openssl s_client -connect bridge.example.com:443 -tls1_1 </dev/null 2>&1 | grep -i "handshake failure"

# Confirm TLS 1.2 is accepted.
openssl s_client -connect bridge.example.com:443 -tls1_2 </dev/null 2>&1 | grep -i "Cipher is"
```

### 3. HTTP redirect

```bash
# Confirm plain HTTP is redirected to HTTPS (not served).
curl -I http://bridge.example.com/slot/test 2>&1 | head -5
```

Expected: `HTTP/1.1 301 Moved Permanently` or `HTTP/2 301`.

### 4. Certificate chain trust

```bash
# Confirm the certificate chain is trusted by the system CA store.
curl --fail https://bridge.example.com/slot/nonexistent 2>&1
```

Expected: `404 Not Found` (not a TLS/certificate error).

### 5. Self-signed certificate rejection

```bash
# Confirm clients reject a self-signed certificate when CA validation is required.
curl --fail --cacert /etc/ssl/certs/ca-certificates.crt https://bridge.example.com/ 2>&1
```

Self-signed certificates are acceptable in controlled test environments only.
They must not be used in production without explicit operator acknowledgement.

### 6. Bridge not reachable on plain HTTP externally

```bash
# Confirm the bridge does not respond to direct plain HTTP from outside the host.
curl --max-time 5 http://bridge.example.com:3000/ 2>&1
```

Expected: connection refused or timeout (not a valid HTTP response).

---

## Release Evidence

Before publishing a release that includes bridge deployment, include the output of the
TLS validation checklist above in the release notes or CI artifacts as release evidence.

Minimum required passing checks for release sign-off:
- Certificate validity confirmed.
- TLS 1.2+ accepted; TLS 1.0/1.1 rejected.
- Plain HTTP redirected to HTTPS.
- Certificate chain trusted.
- Bridge not reachable on plain HTTP from external network.

---

**Secure. Permissioned. Explicit.**
