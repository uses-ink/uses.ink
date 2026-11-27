## 0031 Credentials sealed with XChaCha20-Poly1305

Accepted. Sealed per row with the row id as associated data, so a copied row fails to open. This
replaces the AES-256-GCM suggestion in `research/architecture.md`. **Why:** a 24-byte random nonce
needs no counter; `age` and `hpke` cannot share a build today.
