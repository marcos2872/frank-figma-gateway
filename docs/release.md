# Release de runtime

> Artefatos do LiteLLM/Postgres, manifesto assinado e publicação. O app é leve; os runtimes
> pesados são baixados no onboarding.

## 1. Pré-requisitos

- Runner Windows x64 com Python 3.12 e Node 24.
- Rust stable no job de assinatura.
- Secrets/variables do GitHub:
  - `MANIFEST_SIGNING_KEY`: chave privada Ed25519 em hex (64 caracteres) ou base64 (32 bytes);
  - `MANIFEST_KEY_ID`: identificador da chave, ex. `2026-01`;
  - `MANIFEST_PUBLIC_KEY`: chave pública correspondente em base64.

## 2. Gerar os artefatos

```powershell
./scripts/build-litellm-runtime.ps1 -Version "1.108.1"
./scripts/build-pgsql-runtime.ps1 -Version "16.9" -Build "1"
node scripts/manifest.mjs `
  --litellm "dist/litellm-runtime-1.108.1-win-x64.zip" `
  --pgsql "dist/pgsql-16.9-win-x64.zip" `
  --base-url "https://github.com/<org>/<repo>/releases/download/v0.1.0" `
  --out "dist/manifest.json"
```

O script do LiteLLM usa PyInstaller one-folder e valida `litellm --version`; o do Postgres baixa
os binários oficiais do EDB e valida `postgres --version`. Confirme os hidden imports do LiteLLM
na versão fixada antes de publicar.

## 3. Assinar e verificar

```bash
printf '%s' "$MANIFEST_SIGNING_KEY" > /tmp/manifest.key
cargo run --manifest-path src/Cargo.toml --release -p manifest-signer -- \
  --key /tmp/manifest.key \
  --key-id "$MANIFEST_KEY_ID" \
  --input dist/manifest.json \
  --output dist/manifest.json.sig
rm -f /tmp/manifest.key

cargo run --manifest-path src/Cargo.toml --release -p manifest-verify -- \
  --manifest dist/manifest.json \
  --signature dist/manifest.json.sig \
  --keys src/src-tauri/resources/keys/manifest-keys.json \
  --app-version 0.1.0 \
  --policy src/src-tauri/resources/runtime-policy.json
```

A assinatura cobre os bytes exatos do `manifest.json`. A validação usa `keyId`, status e janela
`notBefore`/`notAfter` (epoch em segundos); o placeholder em `resources/keys` é `retired`.

## 4. Publicar

O workflow [`.github/workflows/release.yml`](../.github/workflows/release.yml) dispara em tags `v*`:

1. build dos dois ZIPs no Windows;
2. geração do `manifest.json`;
3. assinatura e verificação em Ubuntu;
4. publicação do release com ZIPs, manifesto, `.sig` e `SHA256SUMS`.

Não publique sem conferir o `SHA256SUMS` e sem a chave pública correspondente embutida no app.

## 5. Rotação de chave

- Inclua a chave atual e a próxima no `manifest-keys.json` antes de trocar o secret.
- A chave nova precisa estar no app instalado; atualize o app antes de assinar só com ela.
- Chaves antigas viram `retired`; comprometidas viram `revoked`.
