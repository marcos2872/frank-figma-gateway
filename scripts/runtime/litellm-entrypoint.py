"""Entrypoint do LiteLLM empacotado com PyInstaller.

Template copiado para a área de build por scripts/build-litellm-runtime.ps1,
que substitui o marcador de versão abaixo pela versão fixada do runtime.
Manter o código aqui (e não dentro do .ps1) evita que a sintaxe do PowerShell
interprete crases, `$` e afins do Python.
"""

import io
import sys

VERSION = "__LITELLM_VERSION__"


def force_utf8_stdio() -> None:
    """Força UTF-8 em stdout/stderr.

    O banner do LiteLLM usa caracteres de desenho de caixa; no Windows o Python
    congelado abre o pipe como cp1252 e o `click.echo` levanta
    `UnicodeEncodeError`, derrubando o proxy na inicialização. `PYTHONIOENCODING`
    não é respeitado pelo binário congelado, então reconfiguramos aqui.
    """
    for name in ("stdout", "stderr"):
        stream = getattr(sys, name, None)
        buffer = getattr(stream, "buffer", None)
        if buffer is None:
            continue
        setattr(
            sys,
            name,
            io.TextIOWrapper(
                buffer,
                encoding="utf-8",
                errors="replace",
                line_buffering=True,
            ),
        )


def main() -> int:
    # O CLI do LiteLLM não expõe --version; o probe do app espera a versão exata.
    if "--version" in sys.argv[1:]:
        print(f"litellm {VERSION}")
        return 0

    force_utf8_stdio()

    # `litellm.proxy.proxy_server` não exporta `run_server`; o comando Click vive em proxy_cli.
    from litellm.proxy.proxy_cli import run_server

    run_server()
    return 0


if __name__ == "__main__":
    sys.exit(main())
