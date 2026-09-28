# 📑 Plano de Viabilidade Técnica: Integração do OpenCode CLI ao OnyxSH

> **Documento:** Análise de Viabilidade e Arquitetura de Integração  
> **Componente Alvo:** OpenCode CLI (Open-source AI Coding Agent / TUI & Headless)  
> **Sistema Anfitrião:** OnyxSH (Terminal Inteligente GTK4 / Libadwaita)  
> **Data:** Setembro de 2026 | **Status:** Proposta Técnica em Análise  

---

## 1. Sumário Executivo & Veredito

| Métrica | Avaliação |
| :--- | :--- |
| **Veredito de Viabilidade** | 🟢 **Altamente Viável e Estratégico** |
| **Complexidade Técnica** | 🟡 **Média** (Modular em 3 fases bem desacopladas) |
| **Impacto no Usuário** | 🚀 **Muito Alto** (Capacidade de refatoração multi-arquivo, raciocínio em projetos complexos e suporte a MCP) |
| **Risco de Regressão** | 🟢 **Baixo** (Não interfere nos provedores existentes Gemini/Groq/Ollama) |

### Por que esta integração faz sentido?
O **OnyxSH** possui uma infraestrutura de IA excelente para **diagnóstico pontual de erros no shell, geração rápida de scripts e assistência interativa de terminal** (`TerminalAiAssistant`, `SemanticTracker` e `AgentOrchestrator`).

No entanto, para **tarefas complexas de engenharia de software** (análise e refatoração de repositórios inteiros, edição cirúrgica em múltiplos arquivos, árvore sintática e ferramentas externas via Model Context Protocol - MCP), ferramentas especializadas de terminal como o **OpenCode CLI** se destacam.

Integrar o OpenCode CLI ao OnyxSH une o melhor dos dois mundos: o OnyxSH fornece a **interface Libadwaita moderna, abas aceleradas por hardware (VTE), segurança com Production Guard e tracking semântico**, enquanto o OpenCode atua como o **motor avançado de codificação e automação de projetos**.

---

## 2. Diagnóstico da Arquitetura Atual do OnyxSH

O ecossistema de IA do OnyxSH opera através de 4 pilares bem estabelecidos:

1. **Terminal AI Assistant (`src/onyxsh/terminal/ai_assistant.py`):**
   - Painel lateral de chat nativo (`Adw.Bin` / `Gtk.TextView`).
   - Conexão direta com provedores LLM (Gemini, Groq, OpenRouter, Ollama local).
   - Saída estruturada em JSON com campos `"reply"` e `"commands"`, oferecendo botões rápidos de *"Executar no Terminal"* ou *"Inserir no Prompt"*.
2. **Modo Agente Seguro (`src/onyxsh/agent/orchestrator.py`):**
   - Motor com `PlanParser`, `PolicyEngine` (níveis de risco 0 a 4) e `PathGuard` (restrição de escrita e leitura em caminhos sensíveis como `~/.ssh`).
   - Visualização de *Diff* lado a lado antes de aplicar edições em arquivos e rollback com logs de auditoria.
3. **Semantic Tracker & Error Matcher (`src/onyxsh/terminal/semantic_tracker.py`):**
   - Rastreamento OSC 133 para capturar o exato comando executado e sua saída.
   - Detecção proativa de erros (`exit_code != 0`) que exibe o badge *"Analisar com IA"*.
4. **Isolamento de Ambiente e Flatpak:**
   - O OnyxSH detecta execução dentro do Flatpak sandbox e utiliza `flatpak-spawn --host` para interagir com binários e ferramentas da máquina host.

---

## 3. O que é o OpenCode CLI e seus Modos de Operação

O **OpenCode** (open-source AI coding assistant para terminal) oferece 3 modalidades principais de uso:

1. **Modo TUI Interativo (`opencode`):**
   - Interface de terminal rica baseada em texto para dialogar, navegar por diffs e aprovar alterações no projeto.
2. **Modo Headless / Scripting (`opencode run`):**
   - `opencode run "refatore a função X" --format json`
   - Suporta flags como `--model <provider/model>`, `--file <path>`, `--session <id>`, `--continue` e `--agent <name>`.
   - Retorna saída estruturada em JSON sem abrir interface gráfica/TUI.
3. **Modo Servidor / Daemon (`opencode serve`):**
   - Inicia um servidor HTTP/JSON-RPC local que mantém contexto, cache e gerenciamento de sessões persistentes na porta local.

---

## 4. Modelos de Integração Propostos (3 Fases)

```
┌────────────────────────────────────────────────────────────────────────┐
│                                OnyxSH                                  │
├───────────────────────────────────┬────────────────────────────────────┤
│           Camada Visual           │          Camada Headless           │
│                                   │                                    │
│  [Aba / Split Dedicado OpenCode]  │  [Painel Lateral de Chat OnyxSH]   │
│   • VTE Terminal acelerado        │   • Provedor OpenCode Headless     │
│   • TUI completo do OpenCode      │   • opencode run --format json     │
│   • Contexto $PWD atual           │   • Visualização de Diffs nativa   │
├───────────────────────────────────┴────────────────────────────────────┤
│                       Camada de Contexto e Ações                       │
│  • Clique Direito no Terminal ➔ "Refatorar seleção com OpenCode"       │
│  • Badge de Erro OSC 133 ➔ "Resolver repositório com OpenCode"        │
│  • Salvaguarda: Production Guard bloqueia execuções cegas em produção  │
└────────────────────────────────────────────────────────────────────────┘
```

### 🔹 Abordagem 1: "Aba / Split Dedicado OpenCode" (Rápida Adoção & Baixo Risco)
- **Como funciona:**
  - O usuário pressiona um atalho (ex: `Ctrl + Shift + O`), acessa pela **Command Palette** (`Ctrl + Shift + P ➔ "OpenCode: Abrir Assistente de Código"`) ou clica em um botão na barra de ações.
  - O OnyxSH abre uma nova aba ou um split vertical (`Ctrl + Shift + E`) já executando `opencode` diretamente no diretório atual da aba (`$PWD`).
  - No ambiente Flatpak, o comando invocado é automaticamente resolvido como `flatpak-spawn --host opencode`.
- **Vantagens:**
  - Implementação imediata, elegante e de baixíssimo atrito.
  - O usuário aproveita 100% da TUI do OpenCode combinada com as fontes NerdFont, temas e aceleração gráfica do OnyxSH.
  - Sem risco de quebra caso o formato de JSON do OpenCode mude.

---

### 🔹 Abordagem 2: "OpenCode como Provedor Headless no Chat Lateral" (Integração Profunda)
- **Como funciona:**
  - Adicionar o **OpenCode** como uma opção de provedor na lista de IA do OnyxSH (junto de *Gemini, Groq, Ollama, OpenRouter*).
  - Criar `src/onyxsh/agent/providers/opencode.py` herdando de `LLMProvider`.
  - Ao invés de fazer requisições REST diretamente para endpoints web, o OnyxSH invoca em background:
    ```bash
    opencode run --format json --file "<arquivo>" "<prompt do usuário>"
    ```
  - A resposta JSON estruturada é traduzida no `ActionPlan` do OnyxSH, permitindo que as alterações passem pelo visualizador de **Diff nativo em GTK4**, com botões de aceite e reversão segura.
- **Vantagens:**
  - O usuário mantém a experiência limpa e nativa da interface gráfica Libadwaita do OnyxSH.
  - O OpenCode gerencia a busca no repositório e conexões MCP, enquanto o OnyxSH cuida da apresentação, controle de permissões e histórico.

---

### 🔹 Abordagem 3: "Ações Contextuais de Terminal ➔ OpenCode" (Atalhos de Produtividade)
- **Como funciona:**
  - **Menu de Contexto no Terminal:** Ao selecionar um bloco de código ou log no terminal e clicar com o botão direito, exibir a opção: *"Enviar seleção para o OpenCode"*.
  - **Badge de Erro do Semantic Tracker:** Quando um comando de build (`cargo build`, `npm run build`, `make`, `pytest`) falha com `exit_code != 0`, o badge do terminal oferece o botão: `[ Corrigir Projeto com OpenCode ]`, que abre o OpenCode passando o erro como contexto inicial.

---

## 5. Matriz de Segurança e Regras do OnyxSH

| Cenário / Regra | Desafio | Solução no OnyxSH |
| :--- | :--- | :--- |
| **Production Guard** | O OpenCode pode tentar rodar comandos destrutivos em lote. | Em terminais com `ProductionGuard.is_production_terminal() == True`, o OnyxSH deve **bloquear chamadas headless do OpenCode** ou emitir o diálogo de confirmação dupla com digitação do hostname antes de liberar o processo. |
| **Sandbox Flatpak** | Binário `opencode` reside no sistema operacional hospedeiro. | Checar `is_flatpak_sandbox()`. Usar wrapper com `flatpak-spawn --host opencode`. |
| **Presença do Binário** | O usuário pode não ter o OpenCode instalado. | Verificar com `shutil.which("opencode")`. Exibir banner suave com o comando de instalação (`curl -fsSL https://opencode.ai/install \| bash`) e botão de copiar. |
| **PathGuard** | O OpenCode não deve alterar credenciais (`~/.ssh`, `~/.aws`). | Ao operar no modo headless, validar que os caminhos retornados pelo OpenCode respeitam as raízes permitidas do `PathGuard`. |

---

## 6. Cronograma e Esforço Estimado

```
[ Fase 1: Launcher TUI & Command Palette ]  ──▶  ~1 a 2 dias  (Baixo Risco)
[ Fase 2: Provedor Headless no Chat UI ]    ──▶  ~2 a 3 dias  (Médio Risco)
[ Fase 3: Ações de Contexto & Erros ]       ──▶  ~1 a 2 dias  (Baixo Risco)
```

---

## 7. Próximos Passos Recomendados

1. **Validação do Usuário / Veredito:** Confirmar se o foco inicial deve ser a **Abordagem 1 (TUI em Abas/Splits com 1 atalho)** ou a **Abordagem 2 (Provedor Headless integrado ao Chat Lateral)**.
2. **Prototipação da Fase 1:** Criar utilitário `src/onyxsh/utils/opencode_utils.py` com detecção de binário, suporte a Flatpak e injeção na Command Palette.
