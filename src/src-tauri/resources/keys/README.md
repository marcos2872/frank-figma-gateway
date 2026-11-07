# Chaves do manifesto

`manifest-keys.json` contém apenas chaves **públicas** Ed25519 confiadas pelo app.
A chave privada fica no secret `MANIFEST_SIGNING_KEY` do GitHub Actions e nunca é commitada.

O arquivo atual tem um placeholder `retired`; ele não valida nenhum manifesto. Antes de um
release público, substitua-o pela chave pública real e mantenha a privada no secret.

Formato:

```json
[
  {
    "keyId": "2026-01",
    "algorithm": "ed25519",
    "publicKey": "<base64 de 32 bytes>",
    "status": "active",
    "notBefore": 1767225600,
    "notAfter": 1798761600
  }
]
```

`notBefore`/`notAfter` são epoch em segundos. Para rotação, inclua a chave atual e a próxima;
o app só aceita `status: "active"` dentro da janela de validade.
