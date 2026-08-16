# Changelog

Todas as mudanças notáveis no projeto **Zashterminal (Fork)** a partir de 13 de Agosto de 2026 estão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/) e este projeto segue o [Semantic Versioning](https://semver.org/lang/pt-BR/).

---

## [0.8.18] - 2026-08-16

### Adicionado
- **Arquitetura Híbrida de Empacotamento**:
  - `scripts/build_deb.sh`: Criação automatizada de pacotes `.deb` com suporte a `--clean-cache`.
  - `scripts/build_flatpak.sh`: Construção de bundles Flatpak com o manifesto GNOME 46 (`manifests/org.leoberbert.zashterminal.yaml`) e limpeza de cache.
  - `install.sh`: Novos subcomandos `package deb`, `package flatpak`, `package all` e opções de instalação direta de bundles no menu interativo.
- **Pipeline de CI/CD para GitHub Actions**: Workflow `.github/workflows/build-packages.yml` para compilar e anexar `.deb` e `.flatpak` automaticamente nas releases do GitHub.
- **Migração Automática de Configurações no Flatpak**: Importação transparente de configurações do host (`~/.config/zashterminal/`) para o sandbox Flatpak na primeira execução.
- **Detecção de GPU e VRAM no Flatpak**: Consulta de hardware do host via portal `flatpak-spawn --host` (`org.freedesktop.Flatpak`), permitindo que a GPU (NVIDIA `nvidia-smi`, AMD/Intel) e VRAM sejam detectadas perfeitamente em Flatpak.
- **Integração Nativa do Shell do Host no Flatpak**: Execução do shell do usuário (`bash`, `zsh`, `fish`) diretamente no sistema operacional hospedeiro via `flatpak-spawn --host`, habilitando suporte total a elevação de privilégios (`sudo`), gerenciadores de pacotes (`apt`, `pacman`, `dnf`), ferramentas CLI e dotfiles do usuário.

### Corrigido
- **Módulo VTE no Flatpak**: Inclusão e compilação do módulo `vte-0.76` (com suporte a GTK4 e GObject Introspection) no manifesto Flatpak para sanar o erro `Namespace Vte not available`.
- **Localização e Traduções (PT-BR)**: Busca dinâmica de catálogos gettext em `/app/share/locale` e diretórios internos do app, além de compilação explícita para a variante `pt_BR`.
- **Diretório Inicial do Terminal (CWD)**: Correção no launcher `usr/bin/zashterminal` e no spawner para garantir que novas abas e janelas abram no `$HOME` do usuário em vez do caminho interno de sandbox do app.
- **Identificação do Sistema Operacional Hospedeiro na IA**: Resolução automática do `os-release` real do sistema hospedeiro (`/var/run/host/os-release` ou via `flatpak-spawn`), evitando que a IA identifique incorretamente o ambiente como o runtime container do Flatpak.

---

## [0.8.17] - 2026-08-15

### Adicionado
- **Detecção de Hardware e GPU**: Nova função `detect_gpu_info()` em `platform.py` com detecção automática de GPUs NVIDIA (`nvidia-smi`), AMD/Intel (DRM sysfs) ou memória RAM do sistema.
- **Seletor de Janela de Contexto**: Opção na interface do Modo Agente Seguro com dropdown entre 4K, 8K, 16K, 32K, 64K e 128K tokens, recomendando automaticamente o melhor perfil baseado na VRAM detectada.
- **Buffer de Contexto no Ollama (`num_ctx`)**: Repasse dinâmico de `options.num_ctx` no pré-carregamento e nas chamadas de inferência local.
- **Retenção Inteligente de Histórico**: Algoritmo de janela deslizante token-aware no `TerminalAiAssistant` para preservar o histórico de conversas longas sem exceder o limite de contexto do modelo.
- **Ciclo de Vida de VRAM (Pré-carregamento Assíncrono e Liberação)**: Carregamento do modelo local em background no startup do terminal e descarregamento automático da VRAM ao fechar o app.

---

## [0.8.16] - 2026-08-14

### Corrigido
- Escapamento de caracteres especiais (`&`) em títulos de janelas e diálogos para garantir compatibilidade total com o renderizador de markup Pango do GTK4.

---

## [0.8.15] - 2026-08-14

### Adicionado
- Seletor gráfico de níveis de risco do Modo Agente e switches de contexto de sistema (OS/distro/hardware) e diretório de trabalho (`PWD`) no diálogo de configuração da IA.
- Botões de ação rápida para cópia de blocos de scripts e resposta completa do assistente de IA.

---

## [0.8.14] - 2026-08-14

### Corrigido
- Extrator de blocos JSON com suporte a parsing flexível via regex para respostas da IA sem tags markdown strict.
- Aumento do limite de tokens de geração para permitir a emissão de scripts e diagnósticos extensos sem cortes.

---

## [0.8.13] - 2026-08-14

### Corrigido
- Remoção de filtros hardcoded que bloqueavam solicitações legítimas de geração de código e scripts de administração.

---

## [0.8.12] - 2026-08-14

### Melhorado
- Refinamento do prompt de sistema do Assistente de IA para garantir explicações didáticas e ordenação causal lógica de comandos de terminal.

---

## [0.8.11] - 2026-08-14

### Corrigido
- Ajuste de quebra natural de texto (`natural wrap mode`) nos rótulos do GTK4 para evitar avisos de cálculo de layout em redimensionamentos.

---

## [0.8.10] - 2026-08-14

### Corrigido
- Garantia de thread-safety com despachos via `GLib.idle_add` durante o streaming de respostas da IA.
- Adição de handlers globais para logging resiliente de exceções não capturadas.

---

## [0.8.9] - 2026-08-14

### Corrigido
- Correção de chamada de função no status do instalador `install.sh`.

---

## [0.8.8] - 2026-08-13

### Adicionado
- **Início do Fork do Zashterminal**:
  - **Modo Agente Seguro**:
    - Motor de políticas (`PolicyEngine`) estratificado por níveis de risco (Nível 0 a Nível 4).
    - Registro de ferramentas (`ToolRegistry`) para execução controlada e auditada de comandos.
    - Auditoria completa em formato JSONL com trilha de ações executadas pela IA.
    - Suporte a elevação segura via Polkit (`pkexec`) com dry-run e diffs de backup para arquivos de configuração.
  - **Documentação Multilíngue**: `README.md` em português como documentação principal e `README.en.md` em inglês.
