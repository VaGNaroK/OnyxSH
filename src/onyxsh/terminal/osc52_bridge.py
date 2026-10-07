# onyxsh/terminal/osc52_bridge.py
"""
Ponte OSC 52 para integração de clipboard entre aplicações TUI e o desktop.

Permite que aplicações de terminal rodando localmente ou no host (via Flatpak host-spawn),
como OpenCode, Neovim, Tmux, Helix, etc., escrevam diretamente no clipboard do desktop
(GdkClipboard / Wayland / X11) através de sequências de escape padrão OSC 52.

Padrão OSC 52:
    ESC ] 52 ; <target> ; <base64_data> <ST|BEL>
    Exemplo: \033]52;c;SGVsbG8=\007 ou \033]52;c;SGVsbG8=\033\\

Diretrizes de Segurança Zero-Trust:
    1. Operações de leitura (payload '?') são sumariamente REJEITADAS e ignoradas para
       evitar que scripts ou comandos não confiáveis leiam o clipboard do usuário.
    2. Limite rígido de tamanho de payload (MAX_PAYLOAD_BYTES) para prevenir DoS / consumo excessivo.
    3. Tratamento defensivo de codificações inválidas sem travar a thread de leitura do PTY.
"""

import base64
import re
from typing import List, Optional, Tuple

from ..utils.logger import get_logger

logger = get_logger("onyxsh.terminal.osc52_bridge")

# Tamanho máximo de payload permitido por sequência OSC 52 (10 MB)
MAX_PAYLOAD_BYTES = 10 * 1024 * 1024

# Expressão regular para casar sequências OSC 52 completas terminadas em BEL (\x07) ou ST (\x1b\\)
# Grupo 1: Target de seleção (c=clipboard, p=primary, q=secondary, etc., ou vazio)
# Grupo 2: Payload em base64 ou '?' (query)
OSC52_REGEX = re.compile(
    rb"\x1b\]52;([a-zA-Z0-9]*);([A-Za-z0-9+/=\s]*|\?)(?:\x07|\x1b\\)"
)


def parse_osc52_sequences(
    data: bytes,
    max_payload_bytes: int = MAX_PAYLOAD_BYTES,
) -> List[Tuple[str, str]]:
    """
    Analisa dados em bytes e extrai todas as sequências de escrita OSC 52 válidas.

    Args:
        data: Bytes brutos recebidos da stream PTY.
        max_payload_bytes: Limite máximo em bytes do payload base64.

    Returns:
        Lista de tuplas (target, texto_decodificado).
    """
    results: List[Tuple[str, str]] = []

    if b"\x1b]52;" not in data:
        return results

    for match in OSC52_REGEX.finditer(data):
        raw_target = match.group(1).decode("ascii", errors="ignore").lower()
        payload = match.group(2)

        # 1. Regra Zero-Trust: Rejeitar tentativas de leitura do clipboard ('?')
        if payload == b"?":
            logger.debug(
                "OSC 52: Consulta de leitura de clipboard ('?') ignorada por segurança (Zero-Trust)."
            )
            continue

        # 2. Proteção contra DoS: Limite de tamanho
        if len(payload) > max_payload_bytes:
            logger.warning(
                f"OSC 52: Payload excedeu o limite máximo seguro ({len(payload)} > {max_payload_bytes} bytes). Ignorado."
            )
            continue

        # 3. Remover espaços em branco / quebras de linha presentes no base64
        clean_payload = re.sub(rb"\s+", b"", payload)
        if not clean_payload:
            continue

        # 4. Decodificação segura do Base64
        try:
            decoded_bytes = base64.b64decode(clean_payload, validate=False)
            text = decoded_bytes.decode("utf-8", errors="replace")
        except Exception as e:
            logger.debug(f"OSC 52: Erro ao decodificar base64: {e}")
            continue

        # Se target estiver vazio, o padrão do xterm é usar clipboard ('c')
        target = raw_target if raw_target else "c"
        results.append((target, text))

    return results


def strip_osc52_sequences(data: bytes) -> bytes:
    """
    Remove todas as sequências OSC 52 completas dos bytes.
    Impede que sequências de clipboard poluam buffers internos do VTE ou do highlighter.
    """
    if b"\x1b]52;" not in data:
        return data
    return OSC52_REGEX.sub(b"", data)


def apply_osc52_to_clipboard(
    target: str,
    text: str,
    terminal=None,
    clipboard=None,
    primary_clipboard=None,
) -> bool:
    """
    Aplica o texto decodificado ao clipboard e/ou seleção primária do sistema.

    Args:
        target: String de alvos ('c' para clipboard, 'p' para primary).
        text: Texto decodificado em UTF-8.
        terminal: Instância opcional de Vte.Terminal para resolução de clipboard.
        clipboard: Instância opcional de GdkClipboard.
        primary_clipboard: Instância opcional de GdkClipboard primário.

    Returns:
        True se pelo menos um clipboard foi atualizado com sucesso.
    """
    success = False

    # Resolver instâncias de clipboard se não foram passadas diretamente
    if clipboard is None and terminal is not None:
        if hasattr(terminal, "get_clipboard"):
            clipboard = terminal.get_clipboard()
        elif hasattr(terminal, "get_display"):
            disp = terminal.get_display()
            if disp and hasattr(disp, "get_clipboard"):
                clipboard = disp.get_clipboard()

    if primary_clipboard is None and terminal is not None:
        if hasattr(terminal, "get_primary_clipboard"):
            primary_clipboard = terminal.get_primary_clipboard()
        elif hasattr(terminal, "get_display"):
            disp = terminal.get_display()
            if disp and hasattr(disp, "get_primary_clipboard"):
                primary_clipboard = disp.get_primary_clipboard()

    # Fallback global para Gdk.Display padrão se disponível
    if clipboard is None:
        try:
            import gi

            try:
                gi.require_version("Gdk", "4.0")
            except (ValueError, AttributeError):
                pass
            from gi.repository import Gdk

            disp = Gdk.Display.get_default()
            if disp:
                clipboard = disp.get_clipboard()
                if primary_clipboard is None and hasattr(disp, "get_primary_clipboard"):
                    primary_clipboard = disp.get_primary_clipboard()
        except Exception:
            pass

    # Escrever no Clipboard de transferência regular ('c' ou default)
    if "c" in target or not target:
        if clipboard is not None:
            try:
                clipboard.set(text)
                logger.debug(
                    f"OSC 52: {len(text)} caracteres gravados no GdkClipboard com sucesso."
                )
                success = True
            except Exception as e:
                logger.debug(f"OSC 52: Falha ao gravar no clipboard padrão: {e}")

    # Escrever na Seleção Primária ('p')
    if "p" in target:
        if primary_clipboard is not None:
            try:
                primary_clipboard.set(text)
                logger.debug(
                    f"OSC 52: {len(text)} caracteres gravados no Primary Clipboard com sucesso."
                )
                success = True
            except Exception as e:
                logger.debug(f"OSC 52: Falha ao gravar no primary clipboard: {e}")

    return success


def process_osc52_bytes(
    data: bytes,
    terminal=None,
    enabled: bool = True,
) -> bytes:
    """
    Processa a stream de bytes: extrai e aplica comandos OSC 52 ao clipboard,
    retornando os bytes limpos (sem as sequências OSC 52).

    Args:
        data: Bytes lidos do master_fd do PTY.
        terminal: Widget Vte.Terminal associado.
        enabled: Se a ponte OSC 52 está habilitada nas configurações.

    Returns:
        Bytes sanitizados prontos para envio ao VTE.
    """
    if b"\x1b]52;" not in data:
        return data

    if enabled:
        sequences = parse_osc52_sequences(data)
        for target, text in sequences:
            apply_osc52_to_clipboard(target, text, terminal=terminal)

    return strip_osc52_sequences(data)
