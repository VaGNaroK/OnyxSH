# 🏛️ Arquitetura do OnyxSH & Post-Mortem da Tentativa de Integração do OpenCode

> **Documento de Referência Arquitetural e Decisão Técnica (ADR) para Agentes de IA e Desenvolvedores**  
> **Data:** Outubro de 2026  
> **Status:** Decisão Consolidada — Remoção do OpenCode Headless (Chat/SmartRouter) & Manutenção Exclusiva como CLI de Terminal  
> **Módulos Afetados:** `src/onyxsh/agent/`, `src/onyxsh/terminal/`, `src/onyxsh/ui/`, `src/onyxsh/settings/`

---

## 📑 Sumário

1. [Visão Geral da Arquitetura do OnyxSH](#1-visão-geral-da-arquitetura-do-onyxsh)
2. [O Subsistema de IA Original do OnyxSH](#2-o-subsistema-de-ia-original-do-onyxsh)
3. [A Tentativa de Integração do OpenCode](#3-a-tentativa-de-integração-do-opencode)
4. [Por que a Integração Headless Falhou: Análise dos Fatos e Erros](#4-por-que-a-integração-headless-falhou-análise-dos-fatos-e-erros)
   - [4.1. Conflito Fundamental de Paradigmas (Chat vs. Agente ReAct)](#41-conflito-fundamental-de-paradigmas-chat-vs-agente-react)
   - [4.2. Colapso de Tool Calling em Modelos Locais Menores (7B/9B)](#42-colapso-de-tool-calling-em-modelos-locais-menores-7b9b)
   - [4.3. Estouro da Janela de Contexto do Ollama (`n_ctx: 4096`)](#43-estouro-da-janela-de-contexto-do-ollama-n_ctx-4096)
   - [4.4. A Prova Definitiva: O Sucesso da TUI com Modelos em Nuvem](#44-a-prova-definitiva-o-sucesso-da-tui-com-modelos-em-nuvem)
   - [4.5. Latência Desnecessária e Ambiguidade de UX](#45-latência-desnecessária-e-ambiguidade-de-ux)
5. [A Decisão Arquitetural e Plano de Limpeza](#5-a-decisão-arquitetural-e-plano-de-limpeza)
6. [Diretrizes Obrigatórias para Futuros Agentes de IA](#6-diretrizes-obrigatórias-para-futuros-agentes-de-ia)

---

## 1. Visão Geral da Arquitetura do OnyxSH

O **OnyxSH** (`io.github.vagnarok.OnyxSH`) é um emulador de terminal moderno para Linux/GNOME desenvolvido em **Python 3.12+**, **GTK4**, **Libadwaita** e **VTE 3.91+**. A arquitetura do sistema é estruturada em camadas bem definidas:

1. **Camada de Interface Gráfica (`onyxsh.ui`):**
   - Janela principal (`CommTerminalWindow`), abas e divisões (*splits*) gerenciadas por `TabManager`.
   - Painel lateral expansível com o **Assistente de IA** (`AIChatPanel`).
   - Gerenciador de arquivos dual-pane (`FileManagerView`) com visualizador *Quick Look*.
   - Paleta de comandos global (`CommandPaletteDialog`) e diálogo de atalhos.
2. **Camada de Controle e Ciclo de Vida (`onyxsh.terminal`, `onyxsh.app`):**
   - `TerminalManager`: orquestra a criação, eventos, ciclo de vida e renderização dos terminais VTE.
   - `TerminalSpawner`: realiza o bypass da sandbox Flatpak via `host-spawn` / `flatpak-spawn --host`, alocando PTYs reais do sistema host.
   - `SemanticTracker`: monitora sequências OSC 133 (início/fim de comandos, código de retorno) e OSC 7 (diretório de trabalho corrente sincronizado com a UI).
3. **Camada de Infraestrutura e Host (`onyxsh.utils`, host-spawn, Polkit):**
   - Comunicação transparente com o sistema host fora da sandbox Flatpak.
   - Helper com elevação via Polkit para operações administrativas de disco e rede.

---

## 2. O Subsistema de IA Original do OnyxSH

O motor de IA nativo do OnyxSH é projetado para operar com **baixa latência**, **privacidade granular** e **segurança Zero-Trust**:

- **Comunicação Direta via API REST:** Conecta-se diretamente aos endpoints dos provedores (Ollama `/api/chat`, Groq, Google Gemini, Anthropic Claude, OpenAI, OpenRouter) sem intermediários pesados.
- **SmartRouter (`onyxsh.agent.router`):** Analisa a complexidade da mensagem do usuário e direciona para o perfil ideal:
  - `FAST`: consultas curtas, sintaxe de comandos, explicações rápidas (ex: Groq `llama-3.1-8b-instant`).
  - `ADVANCED`: raciocínio detalhado, scripts de automação (ex: Gemini Flash/Pro).
  - `OFFLINE / LOCAL`: execução 100% privada e desconectada na máquina do usuário via Ollama (`localhost:11434`) ou LM Studio.
- **Zero-Trust & Redactor (`onyxsh.agent.redactor`, `path_guard`):** Sanitiza segredos (chaves de API, tokens, senhas) do terminal antes de qualquer envio para provedores de nuvem.

---

## 3. A Tentativa de Integração do OpenCode

O **OpenCode** é uma ferramenta CLI/TUI autônoma de desenvolvimento de software baseada em IA (semelhante ao Claude Code, Cursor CLI ou Aider).

Foi concebido um plano para integrar o OpenCode ao OnyxSH com **dupla finalidade**:
1. **Integração no Terminal (TUI):** Permitir que o usuário abra o OpenCode no terminal do OnyxSH via divisão vertical (`Ctrl + Shift + O`), com detecção de caminho no host e preservação do diretório de trabalho.
2. **Integração Headless no Chat de IA:** Tentar usar o OpenCode CLI como se fosse mais um "provedor de LLM" dentro do `AIChatPanel` e do `SmartRouter`, executando comandos em segundo plano via:
   ```bash
   opencode run --format json "<pergunta_do_usuario>"
   ```

A integração (1) de terminal foi um sucesso. **Porém, a tentativa de integração (2) como provedor headless de chat revelou graves falhas arquiteturais e práticas.**

---

## 4. Por que a Integração Headless Falhou: Análise dos Fatos e Erros

### 4.1. Conflito Fundamental de Paradigmas (Chat vs. Agente ReAct)
- O **Chat de IA do OnyxSH** é um assistente de terminal passivo: o usuário faz uma pergunta ("como ver portas abertas?"), a IA responde com texto e um bloco de código bash com botão "Executar no Terminal".
- O **OpenCode** não é um chatbot de texto; é um **agente autônomo com loop ReAct**. Ele espera ter ferramentas ativas (`read`, `write`, `edit`, `bash`, `explore`), inspecionar múltiplos arquivos do repositório, aplicar patches e pedir confirmação interativa do usuário.
- Ao forçar o OpenCode no modo `run` headless para responder uma pergunta simples de chat, retiramos dele toda a interatividade e forçamos o OnyxSH a tentar decodificar streams NDJSON complexos contendo chamadas de ferramentas internas.

### 4.2. Colapso de Tool Calling em Modelos Locais Menores (7B/9B)
Durante os testes reais utilizando o Ollama local com modelos como `qwen2.5-coder:7b`:
- O OpenCode definiu seu catálogo de ferramentas e enviou para o modelo.
- O modelo local de 7B entendeu que precisava ler um arquivo ou executar um comando shell, mas **não emitiu a chamada de ferramenta estruturada na API do Ollama**. Em vez disso, o modelo gerou a chamada como **texto puro impresso na tela**:
  ```json
  {
    "name": "read",
    "arguments": {
      "path": "src/onyxsh/agent/router.py"
    }
  }
  ```
- O OpenCode recebeu essa saída como texto de resposta final, considerou a tarefa concluída e encerrou a rodada em 6.3 segundos sem jamais ter executado a leitura do arquivo (`read`).
- No chat do OnyxSH, o parser procurava chaves de resposta (`summary`, `reply`, `content`), não as encontrava porque o JSON só continha `name` e `arguments`, e acabava gerando **balões de chat completamente em branco**.

### 4.3. Estouro da Janela de Contexto do Ollama (`n_ctx: 4096`)
Quando testado com o modelo `ornith-1.5:9b`, ocorreu um erro fatal que paralisou a execução:
```text
Error: {"error":{"code":400,"message":"request (5289 tokens) exceeds the available context size (4096 tokens), try increasing it","type":"exceed_context_size_error","n_prompt_tokens":5289,"n_ctx":4096}}
```
**Causa:**
1. O Ollama configura por padrão uma janela de contexto restrita de **4.096 tokens** para modelos locais.
2. O OpenCode injeta dezenas de definições de ferramentas do sistema, prompts de agente, regras de execução, histórico acumulado da sessão e o conteúdo do arquivo a ser lido.
3. A soma ultrapassou 5.289 tokens.
4. O OpenCode tentou acionar o mecanismo de auto-compactação (`Compaction`), mas falhou com:
   > *"The compaction request cannot be reduced further without losing the latest exchange or checkpoint"*.
5. A execução travou completamente.

### 4.4. A Prova Definitiva: O Sucesso da TUI com Modelos em Nuvem
Em contrapartida, quando o usuário abriu o OpenCode diretamente no terminal via divisão de tela e utilizou um dos modelos gratuitos de nuvem inclusos no OpenCode (`Ling 3.1 Flash Free`):
- O modelo executou a ferramenta `read` perfeitamente (`→ Explored: 1 read`).
- A janela de contexto acomodou **12.710 tokens** consumindo apenas **5%** da capacidade total disponível.
- A análise de código foi profunda, detalhada e gerada com raciocínio de 13.6s.
- **Conclusão:** O OpenCode é extraordinário quando roda onde foi projetado para rodar: **na TUI interativa do terminal**, com suporte a modelos preparados para agentes e janelas de contexto amplas.

### 4.5. Latência Desnecessária e Ambiguidade de UX
- **Latência:** Uma consulta direta para o Ollama local (`/api/chat`) ou Groq leva **200 milissegundos**. Chamar o `opencode run` headless leva de **5 a 15 segundos**, pois há overhead de inicialização de daemon, verificação de workspace e compilação de contexto de agente.
- **Confusão de UX:** O usuário via "OpenCode (CLI Local)" no menu suspenso de provedores de IA do chat e nas configurações globais, criando uma sobreposição confusa com o atalho `Ctrl + Shift + O`. Se o usuário selecionasse OpenCode no chat, tinha uma experiência degradada e cheia de falhas; se usasse no terminal, tinha a experiência completa.

---

## 5. A Decisão Arquitetural e Plano de Limpeza

Ficou estabelecida a decisão de **especialização estrita de responsabilidades**:

```
┌─────────────────────────────────────────────────────────────────┐
│                           OnyxSH                                │
├───────────────────────────────┬─────────────────────────────────┤
│    Painel Lateral de IA       │       Terminal VTE (Splits/Abas)│
│  (Perguntas Rápidas & Chat)   │   (Engenharia de Software/TUI)  │
├───────────────────────────────┼─────────────────────────────────┤
│ • APIs REST Diretas e Leves   │ • OpenCode CLI (Ctrl+Shift+O)   │
│ • Ollama / LM Studio (Local)  │ • Modelos Agênticos (Ling, etc.)│
│ • Groq, Gemini, Claude, etc.  │ • Multi-arquivo, Diffs e ReAct  │
│ • Latência: ~200ms            │ • Janela de contexto: 128k+     │
│ • Sem tool-call bloat         │ • Confirmações interativas [Y/n]│
└───────────────────────────────┴─────────────────────────────────┘
```

### Elementos Removidos (Limpeza de Código Inútil):
1. **`src/onyxsh/agent/providers/opencode.py`:** Deletado por completo.
2. **`src/onyxsh/agent/router.py`:** Remoção do enum `RoutingProfile.OPENCODE` e das regras que desviavam mensagens para `provider="opencode"`.
3. **`src/onyxsh/agent/planner.py`:** Remoção de blocos de parsing de JSON específico do OpenCode (L337–425).
4. **`src/onyxsh/ui/widgets/ai_chat_panel.py`:** Remoção de botões de seleção de perfil OpenCode, tags e conversões de NDJSON em texto.
5. **`src/onyxsh/ui/dialogs/ai_config_dialog.py`:** Remoção de "OpenCode (CLI Local)" da lista de provedores de chat, campos de timeout e modelo headless.
6. **`tests/test_opencode_provider.py`:** Removido da suíte de testes.

### Elementos Preservados e Reforçados (Integração CLI Limpa):
1. **Atalho `Ctrl + Shift + O` (`opencode-launcher`):** Abre divisão vertical ou aba dedicada com OpenCode no diretório de trabalho do terminal ativo (via OSC 7).
2. **`onyxsh.utils.opencode_utils`:** Detecção robusta do binário no host, comandos de instalação e bypass do Flatpak sandbox.
3. **Configurações Globais:** Apenas preferências do lançador (`opencode.enabled`, `opencode.binary_path`, `opencode.launch_mode`).

---

## 6. Diretrizes Obrigatórias para Futuros Agentes de IA

Qualquer agente de IA ou desenvolvedor trabalhando neste repositório **DEVE** seguir estas regras:

1. 🚫 **NUNCA tente reintegrar o OpenCode como um provedor de chat ou modelo no SmartRouter:**
   O OpenCode CLI não deve ser chamado via subprocessos com flags headless no fluxo de mensagens do `AIChatPanel`.
2. ⚡ **Mantenha o Chat do OnyxSH focado em APIs Diretas:**
   O `AIChatPanel` e o `TerminalAiAssistant` devem comunicar-se exclusivamente via requisições HTTP REST padronizadas (Ollama, Gemini, Groq, etc.).
3. 🖥️ **Toda integração com ferramentas de desenvolvimento agênticas deve ser feita via Terminal/PTY:**
   Ferramentas como OpenCode, Aider ou Claude Code pertencem ao terminal interativo do usuário, com PTY alocado, suporte a sinais POSIX e interação humana direta.
4. 🧪 **Regra de Validação de Testes:**
   Qualquer alteração em código deve manter 100% dos testes unitários passando:
   ```bash
   PYTHONPATH=src python3 -m unittest discover -s tests
   ```
