"""Entrypoint do LiteLLM empacotado com PyInstaller.

Template copiado para a área de build por scripts/build-litellm-runtime.ps1,
que substitui o marcador de versão abaixo pela versão fixada do runtime.
Manter o código aqui (e não dentro do .ps1) evita que a sintaxe do PowerShell
interprete crases, `$` e afins do Python.
"""

import sys

VERSION = "__LITELLM_VERSION__"


def main() -> int:
    # O CLI do LiteLLM não expõe --version; o probe do app espera a versão exata.
    if "--version" in sys.argv[1:]:
        print(f"litellm {VERSION}")
        return 0

    # `litellm.proxy.proxy_server` não exporta `run_server`; o comando Click vive em proxy_cli.
    from litellm.proxy.proxy_cli import run_server

    run_server()
    return 0


if __name__ == "__main__":
    sys.exit(main())
