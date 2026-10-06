# 📋 Plano de Remoção: OpenCode Headless (Chat/SmartRouter) & Foco na CLI de Terminal

## 🎯 Objetivo
Eliminar a ambiguidade arquitetural e o código redundante/inútil introduzido ao tentar usar o **OpenCode** como um provedor de chat headless (`opencode run`) dentro do painel lateral de IA e do Smart Router. 

Manter o **OpenCode** exclusivamente no papel onde ele é excelente: como um **Assistente de Código Agêntico no Terminal (TUI/CLI)** via divisão de tela (*split pane* ou aba dedicada com `Ctrl + Shift + O`).

---

## 🔍 Diagnóstico de Funções e Componentes Identificados

### 1. 🗑️ Componentes Inúteis / Redundantes (Para Remoção Total)

| Componente / Arquivo | Descrição | Motivo da Remoção |
| :--- | :--- | :--- |
| **`src/onyxsh/agent/providers/opencode.py`** | `OpenCodeProvider` | Tentava executar `opencode run --format json` de forma headless. Gera timeouts desnecessários, balões vazios e trata tool calls como texto de chat. |
| **`tests/test_opencode_provider.py`** | Testes do `OpenCodeProvider` | Testes do provedor headless que será removido. |
| **`src/onyxsh/agent/router.py`** (`RoutingProfile.OPENCODE`) | Perfil de roteamento OpenCode | No chat do OnyxSH, o roteamento só deve ocorrer entre provedores de API leves (`local` via Ollama, `fast` via Groq, `advanced` via Gemini/Claude/OpenAI). OpenCode não é uma API de chat. |
| **`src/onyxsh/agent/planner.py`** (L337–425) | Parser de JSON do OpenCode | Bloco específico que tentava converter JSON emitido pelo `opencode run` em objetos internos de `Plan`. Desnecessário. |
| **`src/onyxsh/settings/config.py`** (campos de headless) | `default_model`, `timeout_seconds`, `offline_provider` | No terminal TUI, o usuário seleciona o modelo via `/model` e não há timeout artificial de subprocesso. |
| **`src/onyxsh/ui/widgets/ai_chat_panel.py`** (UI do Chat) | Botão OpenCode no popover de perfis, label do perfil OpenCode, parsing de NDJSON | O chat não deve listar OpenCode como provedor nem tentar converter eventos de tool calling em mensagens de chat. |
| **`src/onyxsh/ui/dialogs/ai_config_dialog.py`** (Configurações) | Opção "OpenCode (CLI Local)" no dropdown de Provedores de Chat, campos de modelo e timeout headless | Evita confusão no usuário de pensar que o OpenCode é um chatbot de texto comum. |

---

### 2. 🛡️ Componentes Essenciais da Integração CLI (Para Manter e Polir)

| Componente / Arquivo | O que faz | Por que manter? |
| :--- | :--- | :--- |
| **`src/onyxsh/ui/actions.py`** (`opencode_launcher`) | Ação `opencode-launcher` acionada por `Ctrl + Shift + O` | Abre o OpenCode no split vertical ou aba, preservando o diretório de trabalho do terminal ativo via OSC 7. |
| **`src/onyxsh/utils/opencode_utils.py`** | Detecção do binário, host-spawn no Flatpak, verificação de versão | Essencial para saber se o OpenCode está instalado no host e fornecer o comando de instalação caso não esteja. |
| **`src/onyxsh/settings/config.py`** | `opencode.enabled`, `opencode.binary_path`, `opencode.launch_mode` | Permite ao usuário configurar o caminho do binário e escolher se prefere abrir em split vertical ou aba nova. |
| **`src/onyxsh/ui/dialogs/ai_config_dialog.py`** | Grupo de preferências do OpenCode | Exibe o status do binário no host, botão "Verificar", versão detectada e opção de habilitar/desabilitar o lançador. |
| **`src/onyxsh/ui/dialogs/shortcuts_dialog.py`** e **`command_palette_dialog.py`** | Atalho `Ctrl + Shift + O` e item na Paleta de Comandos | Garante descoberta e usabilidade rápida do OpenCode pelo teclado. |

---

## 📐 Plano de Execução Passo a Passo

### Etapa 1: Remoção do Provedor Headless e Limpeza de Dependências
- Deletar o arquivo `src/onyxsh/agent/providers/opencode.py`.
- Atualizar [`src/onyxsh/agent/providers/__init__.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/agent/providers/__init__.py) removendo `OpenCodeProvider`.
- Deletar o arquivo de teste `tests/test_opencode_provider.py`.

### Etapa 2: Limpeza no Smart Router e Planner
- Em [`src/onyxsh/agent/router.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/agent/router.py):
  - Remover `RoutingProfile.OPENCODE`.
  - Remover regras em `resolve_route()` que atribuíam `provider="opencode"`.
  - Manter perfis: `AUTO`, `FAST`, `ADVANCED`, `SECURITY`.
- Em [`src/onyxsh/agent/planner.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/agent/planner.py):
  - Remover bloco de parsing customizado de OpenCode JSON (linhas 337–425).

### Etapa 3: Limpeza no Assistente de Terminal e Painel de Chat
- Em [`src/onyxsh/terminal/ai_assistant.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/terminal/ai_assistant.py):
  - Remover instanciacão do `OpenCodeProvider` em `_get_provider_client()`.
  - Remover checagem de binário do OpenCode em `_validate_route_availability()`.
  - Limpar workarounds de string de OpenCode em `_parse_assistant_payload()`.
- Em [`src/onyxsh/ui/widgets/ai_chat_panel.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/ui/widgets/ai_chat_panel.py):
  - Remover o botão `btn_opencode` do popover de seleção de perfis de roteamento.
  - Remover indicador de perfil `💻 OpenCode`.
  - Limpar tentativas de importação e parsing de NDJSON em `_extract_reply_from_json()`.

### Etapa 4: Limpeza no Diálogo de Configurações de IA
- Em [`src/onyxsh/ui/dialogs/ai_config_dialog.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/ui/dialogs/ai_config_dialog.py):
  - Remover `("opencode", "OpenCode (CLI Local)", "opencode")` de `AI_PROVIDERS`.
  - Remover `"opencode"` do mapa de perfis de roteamento inteligente.
  - No grupo de preferências do OpenCode, remover `opencode_model_row` e `opencode_timeout_row`.
  - Adicionar opção de modo de abertura (`launch_mode`: Split vertical vs. Nova aba).

### Etapa 5: Limpeza das Configurações e Utilitários
- Em [`src/onyxsh/settings/config.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/settings/config.py):
  - Ajustar chave `"opencode"` para conter apenas:
    ```python
    "opencode": {
        "enabled": True,
        "binary_path": "",
        "launch_mode": "split",  # "split" ou "tab"
    }
    ```
- Em [`src/onyxsh/utils/opencode_utils.py`](file:///home/vagnarok/OnyxSH/src/onyxsh/utils/opencode_utils.py):
  - Remover constantes `SETTING_OPENCODE_DEFAULT_MODEL`, `SETTING_OPENCODE_TIMEOUT`, `SETTING_OPENCODE_OFFLINE_PROVIDER`.
  - Preservar funções de resolução de binário, versão e host-spawn.

### Etapa 6: Validação Completa de Testes e Rebuild Flatpak
- Atualizar a suíte de testes unitários (`tests/test_smart_router.py`, `tests/test_opencode_utils.py`, `tests/test_opencode_launcher.py`).
- Executar `PYTHONPATH=src python3 -m unittest discover -s tests` garantindo 100% de aprovação.
- Recompilar e reinstalar o Flatpak: `./scripts/build_flatpak.sh --reinstall`.
