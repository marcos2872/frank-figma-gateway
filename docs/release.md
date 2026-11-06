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

### 1.1 Gerar o par de chaves

O par é gerado fora do repositório: a pública é embutida no app, a privada vai para o GitHub
Secrets.

```powershell
node scripts/generate-manifest-key.mjs `
  --key-id "2026-01" `
  --key-out "$env:USERPROFILE\.ffg\manifest-signing-key.txt"
```

O script grava a chave privada fora do repositório (recusa caminhos internos), substitui o
placeholder em `src/src-tauri/resources/keys/manifest-keys.json` por uma chave `active` com o
`keyId` informado e imprime a chave pública com os comandos de `gh secret set`/`gh variable set`.

Depois de configurar o secret, apague o arquivo da chave privada e mantenha a cópia no cofre.

> A chave pública é compilada dentro do binário (`include_str!` em `src-tauri/src/state.rs`).
> **Recompile o app depois de trocar a chave**: um app antigo recusa o manifesto novo.

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

O runtime tem workflow próprio: [`release-runtime.yml`](../.github/workflows/release-runtime.yml).
Ele dispara em tags `runtime-v*` (ou manualmente por `workflow_dispatch`):

1. build dos dois ZIPs no Windows;
2. geração do `manifest.json`;
3. assinatura e verificação em Ubuntu;
4. publicação do release versionado com ZIPs, manifesto, `.sig` e `SHA256SUMS`;
5. atualização do canal estável `runtime-latest`, que guarda só o manifesto e a assinatura.

```powershell
git tag runtime-v1.108.1-r1
git push origin runtime-v1.108.1-r1
```

**Por que o canal existe:** o app lê
`releases/download/runtime-latest/manifest.json`. Usar `releases/latest` faria o app pegar o
release do app (que não tem manifesto) assim que o CD do app entrar. O manifesto do canal aponta
para os ZIPs do release versionado, então cada versão continua reproduzível e reversível.

O release do app é um workflow separado (tag `v*`), ainda pendente de implementação.

O job de assinatura roda `scripts/check-manifest-key.mjs` antes de assinar e falha se a chave
embutida no app não for a mesma de `MANIFEST_KEY_ID`/`MANIFEST_PUBLIC_KEY`.

Não publique sem conferir o `SHA256SUMS` e sem a chave pública correspondente embutida no app.

## 5. Rotação de chave

- Inclua a chave atual e a próxima no `manifest-keys.json` antes de trocar o secret.
- A chave nova precisa estar no app instalado; atualize o app antes de assinar só com ela.
- Chaves antigas viram `retired`; comprometidas viram `revoked`.
