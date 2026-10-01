# Changelog

Todas as mudanças notáveis no projeto OnyxSH serão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/) e este projeto segue o [Semantic Versioning](https://semver.org/lang/pt-BR/).

## [Unreleased]

### Adicionado
- **Autocomplete browse via `Ctrl + Espaço`**: com prompt vazio lista todos os comandos do catálogo; com comando digitado (ex: `dnf `) lista subcomandos/flags (`CompletionEngine.get_browse_completions()`, `manager._open_browse_completions()`). Popup com barra de rolagem (`Gtk.ScrolledWindow`, altura máxima 420px) para listas longas sem quebrar a apresentação.
- **Spec DNF (Fedora/RHEL)**: `src/onyxsh/terminal/completion/specs/dnf.py` com subcomandos completos e aliases `yum`/`microdnf`, registrado no `SpecRegistry`.
- **Detecção de executáveis do PATH**: `_get_system_completions()` com cache de 60s (Flatpak-safe, só listagem de diretórios), ranqueado abaixo das specs curadas; toggle `autocomplete_system_enabled` + `autocomplete_system_ttl`.
- **Testes**: `tests/test_completion_browse.py`, `tests/test_completion_system.py`, casos DNF em `tests/test_completion_specs.py`.
- **Docs**: seções de browse/PATH/dnf em `docs/MANUAL.md` e `docs/MANUAL.en.md` + atalho na tabela de teclado.

