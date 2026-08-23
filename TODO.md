# 📋 Roadmap & Backlog de Novas Funcionalidades (TODO) — OnyxSH

> **Documento de Planejamento e Backlog Técnico**  
> **Versão:** `0.9.0` | **Data:** 17 de Agosto de 2026  
> **Origem:** Análise de gaps e propostas arquiteturais em `newfeatures.txt`  
> **Critério de Filtragem:** *Todas as funcionalidades já implementadas no OnyxSH foram descartadas. Este documento contém exclusivamente propostas futuras organizadas por prioridade, complexidade técnica e marcos de versão.*

---

## 🎯 Resumo dos Pilares de Evolução

```
                          ┌──────────────────────────────────────┐
                          │          ONYXSH ROADMAP              │
                          └──────────────────┬───────────────────┘
                                             │
      ┌─────────────────┬────────────────────┼───────────────────┬─────────────────┐
      │                 │                    │                   │                 │
┌─────▼──────────┐ ┌────▼──────────┐ ┌───────▼────────┐ ┌────────▼───────┐ ┌──────▼──────────┐
│  1. Core UX &  │ │  2. Onyx      │ │ 3. IA Avançada │ │ 4. File Manager│ │  5. Qualidade &  │
│  Produtividade │ │  Guard (Sec)  │ │ & Onyx Bridge  │ │    2.0 & SFTP  │ │  Confiabilidade  │
└────────────────┘ └───────────────┘ └────────────────┘ └────────────────┘ └──────────────────┘
```

---

## 1. Experiência de Terminal & Produtividade (Core UX)

### 📌 1.1. Command Palette (`Ctrl + Shift + P`)
- [x] **Descrição:** Interface de busca fuzzy centralizada (estilo VS Code / Sublime Text) para invocar qualquer ação do terminal sem navegar por menus.
- [x] **Ações Disponíveis:** Nova aba, divisão horizontal/vertical, alternar temas, abrir configurações, conectar em sessões SSH, abrir SFTP, exportar conversas, limpar histórico, abrir log de auditoria.
- [x] **Status:** ✅ Implementado na versão `0.8.20` (`src/onyxsh/ui/dialogs/command_palette_dialog.py`).
- [x] **Prioridade:** 🔴 Alta | **Esforço:** Médio | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/onyxsh/ui/dialogs/command_palette_dialog.py`, `src/onyxsh/ui/actions.py`, `src/onyxsh/app.py`, `src/onyxsh/settings/config.py`.


### 📌 1.2. Restauração Automática e Inteligente de Sessões (Session Restore)
- [x] **Descrição:** Salvar e restaurar o estado completo da aplicação entre fechamentos e reinicializações.
- [x] **Estado Restaurado:** Abas ativas, layouts em split, diretórios correntes (`$PWD`), sessões SSH abertas (com opção de auto-reconexão), histórico e visibilidade do painel de IA/barras laterais.
- [x] **Modos de Operação nas Preferências:**
  - `Sempre restaurar sessão anterior` (padrão com foco e estado completo).
  - `Perguntar ao iniciar` (toast não-intrusivo de restauração com 1 clique).
  - `Nunca restaurar` (inicia limpo no `$HOME`).
  - Switches de configuração: Auto-reconexão SSH e restauração de painéis da UI.
- [x] **Status:** ✅ Implementado no ciclo `v0.9.0` (`src/onyxsh/state/window_state.py`, `src/onyxsh/terminal/tabs.py`, `src/onyxsh/window.py`, `src/onyxsh/ui/dialogs/preferences_dialog.py`).
- [x] **Prioridade:** 🔴 Alta | **Esforço:** Alto | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/onyxsh/state/window_state.py`, `src/onyxsh/terminal/tabs.py`, `src/onyxsh/window.py`, `src/onyxsh/ui/actions.py`, `src/onyxsh/ui/dialogs/preferences_dialog.py`.

### 📌 1.3. Integração Semântica com Shell (OSC 133 / Semantic Prompts)
- [x] **Descrição:** Suporte a sequências de controle OSC 133 (FinalTerm / iTerm2 semantics) para marcação inteligente de prompts, comandos e saídas do shell.
- [x] **Funcionalidades Habilitadas:**
  - Navegação entre comandos: pular para o prompt anterior (`Alt + Up`) / próximo (`Alt + Down`).
  - Seleção cirúrgica de saída: copiar apenas a saída do último comando executado.
  - Indicador visual de status de saída (ícone de sucesso `0` ou erro `>0` na barra do painel).
  - Medição de tempo de execução por comando (ex: `⏱ 1.4s`).
  - Botão de envio rápido da saída de um comando específico para análise no chat de IA.
- [x] **Status:** ✅ Implementado no ciclo `v0.9.0` (`src/onyxsh/terminal/semantic_tracker.py`, `src/onyxsh/terminal/spawner.py`, `src/onyxsh/terminal/manager.py`, `src/onyxsh/terminal/tabs.py`, `src/onyxsh/ui/actions.py`, `src/onyxsh/ui/dialogs/command_palette_dialog.py`).
- [x] **Prioridade:** 🟡 Média/Alta | **Esforço:** Médio/Alto | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/onyxsh/terminal/semantic_tracker.py`, `src/onyxsh/terminal/spawner.py`, `src/onyxsh/terminal/manager.py`, `src/onyxsh/terminal/tabs.py`, `src/onyxsh/ui/actions.py`, `src/onyxsh/ui/dialogs/command_palette_dialog.py`.

### 📌 1.4. Histórico Enriquecido de Comandos com Busca Fuzzy
- [x] **Descrição:** Persistência estruturada em banco SQLite com metadados detalhados de cada comando executado.
- [x] **Campos:** `command`, `cwd`, `host`, `session_name`, `exit_code`, `duration_ms`, `timestamp`, `is_pinned`, `execution_count`, `last_executed`.
- [x] **Recursos:** Busca fuzzy com ranqueamento, filtros em tempo real (Todos, Diretório Atual `$PWD`, Este Host/Sessão SSH, Favoritos ⭐), inserção no prompt (`Tab`), execução imediata (`Enter`), atalho global `Ctrl + R` e Command Palette.
- [x] **Status:** ✅ Implementado no ciclo `v0.9.0` (`src/onyxsh/data/command_history_manager.py`, `src/onyxsh/ui/dialogs/command_history_dialog.py`, `src/onyxsh/ui/actions.py`, `src/onyxsh/window.py`, `src/onyxsh/terminal/manager.py`).
- [x] **Prioridade:** 🟡 Média | **Esforço:** Médio | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/onyxsh/data/command_history_manager.py`, `src/onyxsh/ui/dialogs/command_history_dialog.py`, `src/onyxsh/ui/actions.py`, `src/onyxsh/window.py`, `src/onyxsh/terminal/manager.py`.

### 📌 1.5. Gerenciador de Snippets de Comandos Reutilizáveis
- [x] **Descrição:** Criação e execução de templates de comandos parametrizáveis com substituição de variáveis.
- [x] **Exemplo:** `docker logs -f {{container_name}} --tail {{lines=100}}` ou `rsync -avz {{cwd}} {{user}}@{{host}}:{{dest=/tmp/}}`.
- [x] **Variáveis Nativas Resolvidas:** `{{cwd}}`, `{{host}}`, `{{user}}`, `{{date}}`, `{{time}}`, `{{datetime}}`, `{{git_branch}}`, `{{clipboard}}`, `{{selection}}`.
- [x] **Recursos:** Diálogo de preenchimento interativo com live preview (`SnippetParameterDialog`), substituição automática de variáveis de sistema, inserção no prompt (`Tab`), execução direta (`Enter`) e indexação na Command Palette (`Ctrl + Shift + P`).
- [x] **Status:** ✅ Implementado no ciclo `v0.9.0` (`src/onyxsh/data/snippet_resolver.py`, `src/onyxsh/ui/dialogs/snippet_parameter_dialog.py`, `src/onyxsh/data/command_manager_models.py`, `src/onyxsh/ui/dialogs/command_palette_dialog.py`).
- [x] **Prioridade:** 🟢 Média | **Esforço:** Baixo/Médio | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/onyxsh/data/snippet_resolver.py`, `src/onyxsh/ui/dialogs/snippet_parameter_dialog.py`, `src/onyxsh/data/command_manager_models.py`, `src/onyxsh/ui/dialogs/command_palette_dialog.py`.

### 📌 1.6. Notificações Desktop para Comandos Longos
- [x] **Descrição:** Notificação nativa via portal XDG/D-Bus (`org.freedesktop.Notifications` / `Gio.Notification`) quando um comando em segundo plano ou em aba inativa terminar após um tempo limite configurável (padrão: > 10s).
- [x] **Informações:** Status visual (`✅ Sucesso (0)` ou `❌ Falha (código)`), nome do comando, tempo de execução (`⏱ duration`), localização (`cwd` / host) e clique interativo que foca e seleciona a aba do terminal.
- [x] **Status:** ✅ Implementado no ciclo `v0.9.0` (`src/onyxsh/terminal/desktop_notifier.py`, `src/onyxsh/terminal/manager.py`, `src/onyxsh/app.py`, `src/onyxsh/ui/dialogs/preferences_dialog.py`).
- [x] **Prioridade:** 🟢 Média | **Esforço:** Baixo | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/onyxsh/terminal/desktop_notifier.py`, `src/onyxsh/terminal/manager.py`, `src/onyxsh/app.py`, `src/onyxsh/ui/dialogs/preferences_dialog.py`.

### 📌 1.7. Busca Avançada no Scrollback e Exportação do Terminal
- [x] **Descrição:** Aprimorar o painel de busca no buffer do terminal com suporte a Regex, correspondência de maiúsculas/minúsculas e exportação direta do buffer/seleção em múltiplos formatos (`.txt`, `.log`, `.md`, `.html` e `.asciinema`).
- [x] **Status:** ✅ Implementado no ciclo `v0.9.0` (`src/onyxsh/terminal/exporter.py`, `src/onyxsh/ui/dialogs/export_dialog.py`, `src/onyxsh/ui/window_ui.py`, `src/onyxsh/window.py`, `src/onyxsh/ui/actions.py`, `src/onyxsh/ui/menus.py`, `src/onyxsh/ui/dialogs/command_palette_dialog.py`).
- [x] **Prioridade:** 🟢 Média | **Esforço:** Baixo/Médio | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/onyxsh/terminal/exporter.py`, `src/onyxsh/ui/dialogs/export_dialog.py`, `src/onyxsh/ui/window_ui.py`, `src/onyxsh/window.py`, `src/onyxsh/ui/actions.py`, `src/onyxsh/ui/menus.py`, `src/onyxsh/ui/dialogs/command_palette_dialog.py`.

### 📌 1.8. Autocomplete Inteligente de Comandos e Sugestões Inline (Ghost Text & Popup Specs)
- [x] **Descrição:** Motor nativo de autocompletar e sugestões preditivas em tempo real no terminal com suporte a dois modos visuais (Texto Fantasma inline estilo Fish/Zsh e Popup Flutuante com descrições estilo Warp/Fig).
- [x] **Fontes de Dados:**
  - 📚 **Histórico Enriquecido (`CommandHistoryManager`)**: Sugestões automáticas baseadas em comandos passados, frequência e contexto de diretório/host.
  - 🛠️ **Catálogo de Specs de Comandos Linux**: Dicionário curado de comandos e subcomandos populares (`sudo`, `apt`, `systemctl`, `journalctl`, `docker`, `git`, `ssh`, `curl`, `tar`, `ufw`, `chmod`, `chown`, etc.) com descrições em linguagem natural.
  - 🧩 **Gerenciador de Snippets (`SnippetManager`)**: Sugestões de templates parametrizados.
- [x] **Mecanismos e Segurança:**
  - Ativação exclusiva em estado de prompt ativo via marcadores semânticos OSC 133 (`PROMPT`), desativando-se automaticamente em editores/ferramentas interativas (`vim`, `nano`, `top`, `fzf`).
  - Aceitação rápida com `Tab`, `Seta Direita` ou `Enter`.
  - Configurações completas nas Preferências (`F2`): Ativar/desativar, escolha de estilo visual e fontes de dados.
- [x] **Status:** ✅ Implementado no ciclo `v0.9.0` (`src/onyxsh/terminal/completion/`, `src/onyxsh/terminal/manager.py`, `src/onyxsh/ui/dialogs/preferences_dialog.py`, `src/onyxsh/settings/config.py`).
- [x] **Prioridade:** 🟢 Média/Alta | **Esforço:** Médio | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/onyxsh/terminal/completion/`, `src/onyxsh/terminal/manager.py`, `src/onyxsh/ui/dialogs/preferences_dialog.py`, `src/onyxsh/settings/config.py`.

### 📌 1.9. Exportação de Sessão com Anotações e Narrativa ("Caderno de Bordo" / Runbook)
- [ ] **Descrição:** Permitir adicionar comentários e anotações explicativas em Markdown intercaladas entre comandos e saídas ao exportar o terminal, gerando documentação técnica automática, runbooks e relatórios post-mortem.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Médio | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/onyxsh/terminal/exporter.py`, `src/onyxsh/ui/dialogs/export_dialog.py`.

### 📌 1.10. Suporte a Gráficos e Imagens no Buffer do Terminal (Protocolo Sixel / iTerm2 Graphics)
- [ ] **Descrição:** Renderizar imagens (PNG, JPEG, SVG) e plotagens gráficas diretamente dentro do buffer do terminal usando protocolo Sixel ou extensões gráficas VTE.
- [ ] **Prioridade:** 🟡 Média | **Esforço:** Médio/Alto | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/onyxsh/terminal/manager.py`, `src/onyxsh/terminal/tabs.py`.

### 📌 1.11. Compartilhamento Instantâneo de Snippets e Saídas via Link Seguro
- [ ] **Descrição:** Gerar links e tokens de compartilhamento de snippets e saídas de comandos; ao abrir o link no OnyxSH, o usuário pode importar diretamente para o gerenciador de snippets ou executá-lo de forma assistida.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Médio | **Alvo:** `v1.0.0`
- [ ] **Módulos Afetados:** `src/onyxsh/data/snippet_resolver.py`, `src/onyxsh/ui/dialogs/command_palette_dialog.py`.

### 📌 1.12. Sessões Compartilhadas & Terminal Multiplayer (Pair Programming / Remote Assist)
- [ ] **Descrição:** Permitir que dois ou mais usuários compartilhem e interajam com a mesma sessão de terminal remoto em tempo real (estilo tmate/Teletype), com indicação visual colorida de quem está digitando.
- [ ] **Prioridade:** 🟡 Média | **Esforço:** Alto | **Alvo:** `v1.1.0`
- [ ] **Módulos Afetados:** `src/onyxsh/terminal/spawner.py`, `src/onyxsh/terminal/manager.py`.

---

## 2. Segurança, DevOps & Infraestrutura ("Onyx Guard & Ops")

### 🛡️ 2.1. Modo Proteção de Produção (Production Guard)
- [x] **Descrição:** Modo de segurança reforçada ativado automaticamente ao conectar em hosts/sessões marcadas como `Produção`.
- [x] **Barreiras de Segurança:**
  - Banner visual permanente vermelho/laranja no topo do terminal indicando `AMBIENTE DE PRODUÇÃO`.
  - Bloqueio e interceptação estrita de comandos de alto risco (`rm -rf`, `mkfs`, `dd`, `systemctl disable/stop`, `shutdown`, `reboot`, `drop database`).
  - Diálogo modal de confirmação dupla com exigência de digitação do nome do host antes de liberar a execução.
  - Bloqueio automático e ofuscação/redação de saídas confidenciais de produção antes do envio para modelos de IA externos.
- [x] **Status:** ✅ Implementado no ciclo `v0.10.0` (`src/onyxsh/terminal/production_guard.py`, `src/onyxsh/ui/widgets/production_banner.py`, `src/onyxsh/ui/dialogs/production_confirm_dialog.py`, `src/onyxsh/sessions/models.py`, `src/onyxsh/terminal/manager.py`, `src/onyxsh/terminal/tabs.py`).
- [x] **Prioridade:** 🔴 Muito Alta | **Esforço:** Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/terminal/production_guard.py`, `src/onyxsh/ui/widgets/production_banner.py`, `src/onyxsh/ui/dialogs/production_confirm_dialog.py`, `src/onyxsh/sessions/models.py`, `src/onyxsh/terminal/manager.py`, `src/onyxsh/terminal/tabs.py`.

### 🛡️ 2.2. Gerenciador Visual de Túneis SSH e Port Forwarding
- [x] **Descrição:** Interface gráfica para criar, monitorar e alternar túneis SSH de forma visual.
- [x] **Modos:**
  - `Local Forwarding (-L)` (ex: acessar banco de dados remoto em `localhost:5432`).
  - `Remote Forwarding (-R)` (ex: expor porta local para o servidor remoto).
  - `Dynamic Forwarding (SOCKS5 Proxy -D)` para navegação segura através do túnel SSH.
- [x] **Recursos:** Indicador de status (🟢 Ativo / 🟡 Conectando / 🔴 Parado / ⚠️ Erro), toggle em 1 clique, auto-reconexão inteligente e ativação automática ao conectar na sessão.
- [x] **Status:** ✅ Implementado no ciclo `v0.10.0` (`src/onyxsh/terminal/tunnel_manager.py`, `src/onyxsh/ui/dialogs/tunnel_manager_dialog.py`, `src/onyxsh/ui/dialogs/tunnel_edit_dialog.py`, `src/onyxsh/sessions/models.py`, `src/onyxsh/terminal/spawner.py`, `src/onyxsh/ui/dialogs/session_edit_dialog.py`).
- [x] **Prioridade:** 🟡 Alta | **Esforço:** Médio/Alto | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/terminal/tunnel_manager.py`, `src/onyxsh/ui/dialogs/tunnel_manager_dialog.py`, `src/onyxsh/ui/dialogs/tunnel_edit_dialog.py`, `src/onyxsh/sessions/models.py`, `src/onyxsh/terminal/spawner.py`.

### 🛡️ 2.3. Health Check e Auto-Reconexão de Sessões SSH
- [ ] **Descrição:** Monitoramento proativo da saúde das conexões remotas.
- [ ] **Recursos:** Detecção imediata de quebra de socket SSH com exibição de banner de aviso e tentativa automática de reconexão (`KeepAlive` inteligente); medição de latência (ping/RTT em ms) exibida na árvore de sessões.
- [ ] **Prioridade:** 🟡 Alta | **Esforço:** Médio | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/onyxsh/sessions/`, `src/onyxsh/ui/sidebar_manager.py`.

### 🛡️ 2.4. Execução em Múltiplos Hosts (Multi-Host Exec / Cluster Commands)
- [ ] **Descrição:** Capacidade de selecionar múltiplos servidores na árvore de sessões e disparar um comando em paralelo.
- [ ] **Recursos:** Saída agrupada por host, visualização de status de sucesso/falha individual, auditoria completa da operação.
- [ ] **Prioridade:** 🟢 Média/Alta | **Esforço:** Alto | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/onyxsh/sessions/`, `src/onyxsh/ui/dialogs/`.

### 🛡️ 2.5. SFTP com Comparação de Diffs Remotos
- [ ] **Descrição:** Integrar o visualizador de diffs do OnyxSH com o cliente SFTP.
- [ ] **Recursos:** Comparar arquivo local com versão remota antes de enviar (`Upload Diff`); comparar versões remotas antes de sobrescrever; backup automático remoto antes da sobrescrita.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Médio | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/onyxsh/filemanager/`, `src/onyxsh/ui/dialogs/diff_review_dialog.py`.

### 🛡️ 2.6. Encadeamento Criptográfico nos Logs de Auditoria (Hash Chain)
- [ ] **Descrição:** Tornar os logs de auditoria à prova de adulteração adicionando SHA-256 encadeado (`previous_hash` + `current_event_hash`) em cada registro de `audit_log.json`.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Baixo/Médio | **Alvo:** `v0.10.0`
- [ ] **Módulos Afetados:** `src/onyxsh/agent/audit_logger.py`.

### 🛡️ 2.7. Dashboard de Métricas de Recursos em Tempo Real (CPU, RAM, Disco, Rede - Local & SSH)
- [ ] **Descrição:** Painel lateral dedicado ou flutuante exibindo gráficos de utilização em tempo real de CPU, memória, disco e rede para a máquina local e sessões SSH remotas conectadas (via `/proc` e `sysstat`).
- [ ] **Prioridade:** 🟡 Média/Alta | **Esforço:** Médio | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/onyxsh/ui/widgets/`, `src/onyxsh/sessions/`, `src/onyxsh/ui/sidebar_manager.py`.

### 🛡️ 2.8. Modo "Leitura Obrigatória" com Justificativa de Auditoria para Produção
- [ ] **Descrição:** Em hosts de produção protegidos pelo Production Guard, exigir que qualquer comando de escrita/modificação passe por uma justificativa textual obrigatória gravada diretamente no log de auditoria antes da execução.
- [ ] **Prioridade:** 🟡 Alta | **Esforço:** Médio | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/onyxsh/terminal/production_guard.py`, `src/onyxsh/agent/audit_logger.py`.

### 🛡️ 2.9. Detecção Proativa de Comportamento Anômalo no Terminal (Anomaly Detection)
- [ ] **Descrição:** Motor estatístico e comportamental que monitora comandos em tempo real, gerando alertas visuais preventivos contra anomalias (horários incomuns de operação, padrões destrutivos em massa, repetidas falhas de autenticação).
- [ ] **Prioridade:** 🟡 Média | **Esforço:** Alto | **Alvo:** `v1.1.0`
- [ ] **Módulos Afetados:** `src/onyxsh/terminal/production_guard.py`, `src/onyxsh/agent/audit_logger.py`.

### 🛡️ 2.10. Integração com Gerenciadores de Segredos e Cofres (1Password, Bitwarden, pass)
- [ ] **Descrição:** Integração nativa com cofres de senhas e gerenciadores de segredos para busca e preenchimento seguro de senhas, chaves privadas SSH e tokens de API com consentimento explícito do usuário.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Médio | **Alvo:** `v1.0.0`
- [ ] **Módulos Afetados:** `src/onyxsh/sessions/`, `src/onyxsh/settings/config.py`, `src/onyxsh/terminal/spawner.py`.

---

## 3. IA Avançada, Agente Autônomo & Extensibilidade ("Onyx Agent & Bridge")

### 🤖 3.1. Modo Interativo "Plan before Execute"
- [x] **Descrição:** Quando o agente de IA sugerir uma tarefa de múltiplos passos, gera uma árvore de plano visual interativa no painel de chat com controle total antes e durante a execução.
- [x] **Recursos:**
  - Painel de controle em lote (`.ai-plan-control-bar`) com contagem de etapas, badge de risco máximo consolidado e barra de progresso visual em tempo real.
  - **Aprovação Granular e Modos de Lote:**
    - 🚀 **Executar Tudo:** Execução sequencial assíncrona de todas as etapas selecionadas e não bloqueadas com feedback dinâmico.
    - 🟢 **Apenas Diagnósticos:** Filtra e executa com segurança apenas as etapas de leitura (`RiskLevel.READ_ONLY` / Nível 0) sem modificar o sistema.
    - 👁️ **Passo a Passo:** Execução guiada etapa por etapa com revisão e confirmação interativa.
    - ⏹️ **Parar:** Interrupção imediata da fila de lote em andamento.
  - **Seleção Granular por Checkbox:** Checkboxes individuais em cada etapa para incluir/excluir comandos antes do disparo.
  - **Indicadores de Estado e Lifecycle:** Rastreamento dinâmico de status por etapa (`⚪ Pendente`, `🟡 Executando...`, `🟢 Concluído`, `🔴 Falha`, `⏭️ Ignorado`) com spinners visuais.
  - **Compatibilidade e Fallbacks Multi-Provedor:** Suporte tanto para modelos estruturados com JSON Schema (Gemini, Claude) quanto fallback de extração de blocos markdown ````bash```` para LLMs locais compactos (Ollama, LM Studio).
- [x] **Prioridade:** 🔴 Muito Alta | **Esforço:** Alto | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/agent/models.py`, `src/onyxsh/agent/planner.py`, `src/onyxsh/ui/widgets/ai_chat_panel.py`, `src/onyxsh/data/styles/ai_chat_panel.css`, `tests/test_plan_execution.py`.

### 🤖 3.2. Verificação Pós-Execução Automatizada (Post-Verification Loop & Auto-Fix)
- [x] **Descrição:** Motor inteligente de inferência e testes de sanidade pós-execução. Quando o assistente ou o usuário executa comandos que alteram o estado do sistema, o agente infere e propõe/executa automaticamente verificações de validação com diagnósticos em caso de erro.
- [x] **Regras de Inferência Curadas (`PostVerifier`):**
  - 🔄 **Serviços Systemd/SysV:** `systemctl (start|restart|reload)` -> `systemctl is-active <svc>` (diagnóstico com `journalctl -u <svc> -n 25 --no-pager` em caso de falha); `systemctl enable` -> `systemctl is-enabled <svc>`.
  - 🌐 **Web Servers & Proxies:** `nginx -t` para Nginx; `apache2ctl configtest` / `httpd -t` para Apache; `sshd -t` para SSH daemon.
  - 🛡️ **Firewalls:** `ufw status verbose` para UFW; `iptables -L -n -v` para iptables.
  - 📁 **Permissões e Arquivos:** `chmod`/`chown`/`chgrp` -> `ls -ld <path>`; `mkdir` -> `test -d <dir>`; `touch`/`cp`/`mv` -> `test -e <path>`; `rm` -> `test ! -e <path>`.
  - 📦 **Gerenciadores de Pacotes:** `apt`/`dpkg` -> `dpkg -s <pkg>`; `dnf`/`yum` -> `rpm -q <pkg>`; `pip` -> `pip show <pkg>`.
  - 🐳 **Contêineres:** `docker run/start` -> `docker ps -f name=<container>`; `docker compose up` -> `docker compose ps`.
  - ⏰ **Crontab:** `crontab -l`.
- [x] **Cartão Visual de Sanidade (`.ai-verification-card`):**
  - Exibição integrada no chat com lista de validações recomendadas, badges de status (`⏳ Aguardando`, `🟡 Validando...`, `🟢 Sanidade Confirmada`, `🔴 Falha na Validação`) e botão de disparo `⚡ Validar Agora`.
  - **Loop de Auto-Correção com IA (`🤖 Diagnosticar e Corrigir com IA`):** Captura automática de logs de erro do `journalctl` ou teste de sintaxe e despacho direto para a IA analisar a causa raiz e propor um plano de reparo em 1 clique.
  - **Preferências do Usuário:** Opção para sugerir verificações e switch para execução automática (`ai_agent_auto_verify`).
- [x] **Prioridade:** 🟡 Alta | **Esforço:** Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/agent/verifier.py`, `src/onyxsh/ui/widgets/ai_chat_panel.py`, `src/onyxsh/data/styles/ai_chat_panel.css`, `src/onyxsh/settings/config.py`, `src/onyxsh/ui/dialogs/ai_config_dialog.py`, `tests/test_post_verification.py`.

### 🤖 3.3. Roteamento Inteligente de Provedores de IA (Smart Model Routing)
- [x] **Descrição:** Permitir associar diferentes modelos de IA conforme a complexidade da tarefa.
- [x] **Perfis e Roteamento:**
  - *⚡ Perfil Rápido / Sintaxe:* Modelo rápido / local (Groq `llama-3.1-8b-instant` / Ollama `llama3.2`).
  - *🧠 Perfil Avançado / Planejamento:* Modelo avançado com raciocínio profundo (Gemini `gemini-2.5-flash` / Claude 3.5 Sonnet / OpenRouter).
  - *🛡️ Perfil Segurança / Diagnóstico:* Modelo forte com raciocínio focado em auditoria e auto-correção (*Self-Healing*).
  - *Classificador Heurístico em Tempo Real (`TaskComplexityClassifier`):* Analisa o prompt em tempo real e escolhe automaticamente entre o perfil Rápido e o Avançado.
  - *Seletor Visual Dinâmico no Chat (`Gtk.MenuButton`):* Alternador no cabeçalho do painel de chat com opções `🔄 Auto`, `⚡ Rápido` e `🧠 Avançado`.
  - *Gerenciamento Multi-Chave:* Armazenamento independente de API keys por provedor (`ai_api_key_gemini`, `ai_api_key_groq`, `ai_api_key_openrouter`).
- [x] **Prioridade:** 🟡 Média/Alta | **Esforço:** Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/agent/router.py`, `src/onyxsh/terminal/ai_assistant.py`, `src/onyxsh/settings/config.py`, `src/onyxsh/ui/dialogs/ai_config_dialog.py`, `src/onyxsh/ui/widgets/ai_chat_panel.py`, `tests/test_smart_router.py`.

### 🤖 3.4. Modo Estritamente Offline / Local-Only com Indicador Visual
- [x] **Descrição:** Chave global de privacidade que desativa qualquer saída para provedores externos de IA (Gemini, Groq, OpenRouter), forçando o uso exclusivo de modelos locais via Ollama/LocalAI e exibindo um selo visual interativo `🛡️ Offline (Local)` no cabeçalho do chat, no diálogo de configurações e na Command Palette.
- [x] **Prioridade:** 🟡 Alta | **Esforço:** Baixo/Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/ui/dialogs/ai_config_dialog.py`, `src/onyxsh/terminal/ai_assistant.py`, `src/onyxsh/ui/widgets/ai_chat_panel.py`, `src/onyxsh/ui/dialogs/command_palette_dialog.py`.

### 🤖 3.5. Integrações Específicas com Git (Assistente de Commit & Auditoria de Segredos)
- [x] **Descrição:** Assistente especializado em fluxos de trabalho com Git.
- [x] **Recursos:** Gerador de mensagens de commit inteligentes no padrão **Conventional Commits** (`feat:`, `fix:`, `refactor:`, `docs:`, etc.) baseadas no `git diff --cached` / staged, auditoria e detecção proativa de segredos/chaves de API pré-commit, interface gráfica modal Libadwaita (`GitCommitDialog`), opções de estilo (Conventional, Resumido, Detalhado), botões de estagiar/desestagiar tudo e integração com a Command Palette (<kbd>Ctrl + Shift + P</kbd>).
- [x] **Prioridade:** 🟢 Média | **Esforço:** Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/utils/git_utils.py`, `src/onyxsh/terminal/git_assistant.py`, `src/onyxsh/ui/dialogs/git_commit_dialog.py`, `src/onyxsh/ui/actions.py`, `src/onyxsh/ui/dialogs/command_palette_dialog.py`, `tests/test_git_assistant.py`.

### 🔌 3.6. API de Extensibilidade e Plugins ("Onyx Bridge")
- [ ] **Descrição:** Framework de extensões em Python que permite à comunidade criar plugins modulares.
- [ ] **Capacidades dos Plugins:**
  - Registrar novas ferramentas seguras para o agente de IA (`AgentTool`).
  - Adicionar itens na Command Palette.
  - Criar novos painéis laterais e conectores de nuvem (AWS, Docker, Kubernetes).
  - Adicionar temas e regras de sintaxe personalizadas.
- [ ] **Prioridade:** 🟡 Média/Alta | **Esforço:** Alto | **Alvo:** `v1.0.0`
- [ ] **Módulos Afetados:** `src/onyxsh/plugins/` (novo pacote).

### 🤖 3.7. Sugestão de Correção Proativa e Ações Rápidas para Erros Comuns de Terminal
- [ ] **Descrição:** Ao detectar saídas de erro conhecidas no terminal (ex: `command not found`, `permission denied`, `port already in use`, `address already bound`, `no space left on device`), o assistente exibe uma notificação sutil ou botão com 1 clique para auto-correção sugerida pela IA.
- [ ] **Prioridade:** 🟡 Alta | **Esforço:** Médio | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/onyxsh/agent/verifier.py`, `src/onyxsh/terminal/semantic_tracker.py`, `src/onyxsh/ui/widgets/ai_chat_panel.py`.

### 🤖 3.8. Catálogo de "Receitas" Guiadas do Agente com Validação de Pré-requisitos & Rollback
- [ ] **Descrição:** Catálogo curado de receitas de automação em YAML/JSON (ex: "Instalar Docker Engine", "Configurar Nginx com SSL Let's Encrypt", "Hardening de SSH") com validação de pré-requisitos (SO, pacotes, portas livres), execução assistida passo a passo e capacidade de rollback em falhas.
- [ ] **Prioridade:** 🟡 Média | **Esforço:** Médio/Alto | **Alvo:** `v1.0.0`
- [ ] **Módulos Afetados:** `src/onyxsh/agent/planner.py`, `src/onyxsh/agent/models.py`, `src/onyxsh/ui/dialogs/`.

### 🤖 3.9. Modo "Aprendizado por Demonstração" (Workflow Learning & Snippet Synthesis)
- [ ] **Descrição:** Gravar uma sessão manual de trabalho do usuário e utilizar o agente para sintetizar e rotular automaticamente novos templates de snippets parametrizados ou receitas reutilizáveis para a equipe.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Alto | **Alvo:** `v1.1.0`
- [ ] **Módulos Afetados:** `src/onyxsh/agent/planner.py`, `src/onyxsh/data/snippet_resolver.py`.

---

## 4. Qualidade, Testes & Diagnóstico do Sistema

### 🧪 4.1. Expansão da Suíte de Testes Automatizados
- [ ] **Meta:** Elevar a cobertura de testes para os fluxos críticos de ponta a ponta.
- [ ] **Novas Suítes de Teste Necessárias:**
  - `test_path_traversal_attacks.py`: Validação exaustiva contra tentativas de bypass em `PathGuard`.
  - `test_untrusted_injection.py`: Testes de prompt injection e encapsulamento em tags `<untrusted>`.
  - `test_session_restore.py`: Testes unitários para serialização e desserialização de layouts.
  - `test_host_spawn_integration.py`: Testes de fallback e execução via `host-spawn`.
- [ ] **Prioridade:** 🔴 Muito Alta | **Esforço:** Médio/Alto | **Alvo:** Contínuo (`v0.9.0` -> `v1.0.0`)
- [ ] **Módulos Afetados:** `tests/`.

### 🧪 4.2. Modo Diagnóstico Seguro (`onyxsh --diagnose`)
- [x] **Descrição:** Comando CLI que gera um relatório técnico sanitizado do sistema (distro, Wayland/X11, versão do GTK/VTE, GPU, runtime Python, permissões Flatpak, subsistema de IA e logs recentes sem dados pessoais) para agilizar suporte e abertura de issues no GitHub.
- [x] **Recursos Implementados:**
  - CLI com suporte a `--diagnose` / `--diagnostics`, `--json`, `--output <arquivo>` e `--lines <N>`.
  - Sanitização em cascata de credenciais/chaves, IPs, emails e caminhos `/home/<user>`.
  - Checagem ao vivo de conectividade do Ollama local.
  - Diálogo modal Libadwaita na GUI (`SystemDiagnosticsDialog`) com visualizador monospace, cópia para clipboard e salvamento em arquivo.
  - Acesso via Preferências > Avançado e Command Palette (<kbd>Ctrl + Shift + P</kbd>).
- [x] **Status:** ✅ Implementado no ciclo `v0.10.0` (`src/onyxsh/utils/diagnostics.py`, `src/onyxsh/ui/dialogs/diagnostics_dialog.py`, `src/onyxsh/ui/actions.py`, `src/onyxsh/ui/dialogs/preferences_dialog.py`, `src/onyxsh/ui/dialogs/command_palette_dialog.py`, `tests/test_diagnostics.py`).
- [x] **Prioridade:** 🟢 Média | **Esforço:** Baixo/Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/utils/diagnostics.py`, `src/onyxsh/ui/dialogs/diagnostics_dialog.py`, `src/onyxsh/ui/actions.py`, `src/onyxsh/ui/dialogs/preferences_dialog.py`, `src/onyxsh/ui/dialogs/command_palette_dialog.py`, `tests/test_diagnostics.py`.

### 🧪 4.3. Relatório Interativo de Saúde do Sistema em HTML (System Health Report)
- [ ] **Descrição:** Ampliar a suíte do `--diagnose` para gerar relatórios HTML autônomos e interativos com gráficos de recursos, inventário de pacotes atualizáveis, checagem de vulnerabilidades conhecidas e recomendações de otimização do sistema.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Médio | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/onyxsh/utils/diagnostics.py`, `src/onyxsh/ui/dialogs/diagnostics_dialog.py`.

### 🧪 4.4. Modo "Sombra" e Execução em Simulação (Shadow Mode / Dry-Run Sandbox)
- [ ] **Descrição:** Ambiente seguro de simulação e dry-run que intercepta chamadas de modificação para calcular e exibir um diff prévio das alterações de disco antes de aplicá-las em ambiente real.
- [ ] **Prioridade:** 🟡 Média | **Esforço:** Alto | **Alvo:** `v1.1.0`
- [ ] **Módulos Afetados:** `src/onyxsh/terminal/production_guard.py`, `src/onyxsh/agent/planner.py`.

---

## 5. Gerenciador de Arquivos Integrado & SFTP/SSH Explorer (File Manager 2.0)

### 📌 5.1. Pré-visualização Rápida de Arquivos (*Quick Look* / Ação no Menu & Diálogo de Inspeção)
- [x] **Descrição:** Permitir pré-visualizar rapidamente o conteúdo de arquivos selecionados sem abrir editores externos. Ação de "Quick Look" no menu de contexto abre um diálogo Libadwaita moderno com renderização rica.
- [x] **Formatos Suportados:**
  - 📝 **Código e Scripts:** Realce de sintaxe (*syntax highlighting*) para `.sh`, `.py`, `.json`, `.yaml`, `.yml`, `.conf`, `.toml`, `.md`, `.c`, `.rs`.
  - 📜 **Logs de Sistema:** Visualização de `.log` com destaque de linhas de erro (`ERROR`, `FATAL`, `FAIL`) e avisos (`WARN`).
  - 🖼️ **Imagens e Mídias:** Pré-visualização de `.png`, `.jpg`, `.svg`, `.ico`, `.webp` com dimensões e tamanho.
  - 🔐 **Certificados e Chaves:** Exibição estruturada de metadados de `.crt`, `.pem`, `.pub`.
- [x] **Status:** ✅ Implementado no ciclo `v0.10.0` (`src/onyxsh/filemanager/quick_look.py`, `src/onyxsh/filemanager/manager.py`, `src/onyxsh/utils/syntax_utils.py`).
- [x] **Prioridade:** 🔴 Alta | **Esforço:** Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/filemanager/manager.py`, `src/onyxsh/filemanager/quick_look.py`, `src/onyxsh/utils/syntax_utils.py`.

### 📌 5.2. Ações Rápidas de Terminal no Menu de Contexto
- [x] **Descrição:** Enriquecer o menu de contexto (botão direito) dos arquivos e pastas com atalhos diretos e inteligentes para o terminal ativo.
- [x] **Ações Implementadas:**
  - 📋 **Copiar Caminho Absoluto:** Copia o path formatado (`/home/user/projeto/arquivo.py`) para a área de transferência do sistema (com suporte a múltiplos arquivos).
  - ⌨️ **Inserir Caminho no Prompt:** Insere o caminho escapado (`shlex.quote`) diretamente na linha de comando do terminal ativo.
  - ▶️ **Executar no Terminal:** Insere e executa automaticamente `./script.sh` ou `python3 arquivo.py` no terminal conectado para diversos interpretadores (`python3`, `bash`, `node`, `ruby`, `perl`, binários).
  - 🛡️ **Executar com `sudo`:** Injeta e executa `sudo ./script.sh` com elevação de privilégios.
  - 📜 **Acompanhar Log em Tempo Real (`tail -f`):** Executa `tail -n 50 -f arquivo.log` diretamente na sessão ativa do terminal.
  - 📂 **Abrir no Terminal (`cd`):** Navega o terminal ativo para o diretório selecionado.
- [x] **Status:** ✅ Implementado no ciclo `v0.10.0` (`src/onyxsh/filemanager/manager.py`, `scripts/sync_translations.py`).
- [x] **Prioridade:** 🔴 Alta | **Esforço:** Baixo/Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/filemanager/manager.py`, `scripts/sync_translations.py`.

### 📌 5.3. Integração Inteligente com a IA do OnyxSH
- [x] **Descrição:** Conectar o gerenciador de arquivos diretamente ao subsistema de IA do OnyxSH para análise contextual de código, logs e segurança.
- [x] **Recursos:**
  - 💡 **"Explicar este arquivo com IA":** Envia o conteúdo do script/arquivo selecionado para o painel de chat de IA com uma explicação detalhada e didática de seu funcionamento.
  - 🔍 **"Diagnosticar Erros com IA":** Lê as últimas linhas de falha de um `.log` e gera sugestões automatizadas de correção.
  - 🛡️ **"Auditar Permissões & Segurança com IA":** Avalia riscos de segurança em arquivos de configuração (`sshd_config`, `sudoers`, `.env`, permissões abertas `777`).
- [x] **Status:** ✅ Implementado no ciclo `v0.10.0` (`src/onyxsh/filemanager/manager.py`, `src/onyxsh/filemanager/quick_look.py`, `src/onyxsh/ui/widgets/ai_chat_panel.py`, `src/onyxsh/ui/window_ui.py`, `tests/test_filemanager_ai.py`).
- [x] **Prioridade:** 🟡 Média/Alta | **Esforço:** Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/filemanager/manager.py`, `src/onyxsh/filemanager/quick_look.py`, `src/onyxsh/ui/widgets/ai_chat_panel.py`, `src/onyxsh/ui/window_ui.py`, `scripts/sync_translations.py`, `tests/test_filemanager_ai.py`.

### 📌 5.4. Atalhos Rápidos & Bookmarks de Diretórios (*Quick Jump*)
- [x] **Descrição:** Menu de salto rápido e favoritos integrado ao lado da barra de navegação (*breadcrumb*) para navegação instantânea em pastas frequentes.
- [x] **Atalhos Nativos:**
  - 🏠 **Home** (`~` / `$HOME`)
  - 📁 **Raiz do Sistema** (`/`)
  - 📜 **Logs do Sistema** (`/var/log`)
  - ⚙️ **Configurações** (`/etc` ou `~/.config`)
  - 🚀 **Raiz do Projeto Git** (detecta o repositório git atual dinamicamente)
  - 💾 **Diretório Temporário** (`/tmp`)
  - ⭐ **Favoritos Personalizados:** Permitir fixar (*pin*) diretórios remotos SSH ou locais frequentes.
- [x] **Status:** ✅ Implementado no ciclo `v0.10.0` (`src/onyxsh/filemanager/manager.py`, `src/onyxsh/settings/config.py`, `src/onyxsh/settings/manager.py`, `tests/test_filemanager_bookmarks.py`).
- [x] **Prioridade:** 🟡 Média | **Esforço:** Baixo/Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/filemanager/manager.py`, `src/onyxsh/settings/config.py`, `src/onyxsh/settings/manager.py`.

### 📌 5.5. Barra de Status, Contadores e Espaço Livre em Disco
- [x] **Descrição:** Exibir informações e métricas em tempo real na barra inferior do gerenciador de arquivos.
- [x] **Métricas Exibidas:**
  - Contagem total de itens no diretório (ex: `48 itens`).
  - Contagem e peso dos itens selecionados (ex: `3 arquivos, 2 pastas selecionados (1.4 MB)`).
  - Espaço livre em disco no ponto de montagem atual (local ou remoto via SSH `df -h`).
- [x] **Status:** ✅ Implementado no ciclo `v0.10.0` (`src/onyxsh/filemanager/manager.py`, `src/onyxsh/filemanager/models.py`, `tests/test_filemanager_status_bar.py`).
- [x] **Prioridade:** 🟢 Média | **Esforço:** Baixo | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/filemanager/manager.py`, `src/onyxsh/filemanager/models.py`.

### 📌 5.6. Badges Visuais, Destaque de Permissões e Symlinks
- [x] **Descrição:** Modernizar a visualização das colunas da tabela de arquivos com chips e cores semânticas.
- [x] **Indicadores Visuais:**
  - 🟢 **Badge Verde para Executáveis:** Destacar scripts e binários com permissão de execução (`+x` / `rwxr-xr-x`).
  - 🔒 **Ícone e Badges de Logs e Configurações:** Identificação visual inteligente para arquivos `.log`, `.conf`, `.yml`, `.json`.
  - 🔗 **Indicação de Links Simbólicos:** Exibir o caminho de destino de links simbólicos (`symlink ➔ /alvo`).
- [x] **Status:** ✅ Implementado no ciclo `v0.10.0` (`src/onyxsh/filemanager/models.py`, `src/onyxsh/filemanager/manager.py`).
- [x] **Prioridade:** 🟢 Média | **Esforço:** Baixo/Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/filemanager/models.py`, `src/onyxsh/filemanager/manager.py`.

### 📌 5.7. Modo Dual-Pane Local ⇄ Remoto para Sessões SSH
- [ ] **Descrição:** Ao conectar em uma sessão SSH remota, permitir dividir o gerenciador de arquivos em dois painéis lado a lado (Painel Local à esquerda e Painel Remoto SSH à direita).
- [ ] **Recursos:**
  - Transferência bidirecional com arrastar e soltar (*drag-and-drop*) entre painéis.
  - Botões dedicados de upload (`->`) e download (`<-`).
  - Sincronização e visualização de diffs de arquivos entre local e servidor.
- [ ] **Prioridade:** 🟡 Média | **Esforço:** Alto | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/onyxsh/filemanager/manager.py`, `src/onyxsh/filemanager/transfer_manager.py`, `src/onyxsh/terminal/tabs.py`.

### 📌 5.8. Verificador Visual e Comparador de Checksums / Hash
- [x] **Descrição:** Interface visual moderna e utilitário no terminal para cálculo assíncrono e verificação de integridade de arquivos locais e remotos.
- [x] **Recursos:**
  - 🔐 **Cálculo Multialgoritmo Assíncrono:** Cálculo sem travamento de UI com barra de progresso para `SHA-256`, `SHA-512`, `MD5` e `SHA-1`.
  - 🎯 **Comparador Inteligente (Hash Matcher):** Campo de validação em tempo real que compara o hash esperado colado pelo usuário (ignorando case/espaços e auto-detectando o algoritmo) e exibe selo de autenticidade (Verde: Match / Vermelho: Divergência).
  - 📋 **Copiar Hashes:** Cópia individual com 1 clique de cada hash ou relatório completo formatado.
  - ⌨️ **Integração com Terminal:** Injeta `sha256sum arquivo` diretamente no prompt do terminal conectado.
  - 🌟 **Integração com File Manager & Quick Look:** Ação dedicada no menu de contexto e botão de hash no Quick Look.
- [x] **Status:** ✅ Implementado no ciclo `v0.10.0` (`src/onyxsh/utils/checksum_utils.py`, `src/onyxsh/ui/dialogs/checksum_dialog.py`, `src/onyxsh/filemanager/manager.py`, `src/onyxsh/filemanager/quick_look.py`, `tests/test_checksum.py`).
- [x] **Prioridade:** 🟡 Média | **Esforço:** Baixo/Médio | **Alvo:** `v0.10.0`
- [x] **Módulos Afetados:** `src/onyxsh/ui/dialogs/checksum_dialog.py`, `src/onyxsh/utils/checksum_utils.py`, `src/onyxsh/filemanager/manager.py`, `src/onyxsh/filemanager/quick_look.py`, `scripts/sync_translations.py`, `tests/test_checksum.py`.

### 📌 5.9. Visualização em Árvore Hierárquica (Tree View) com Métricas Recursivas de Disco
- [ ] **Descrição:** Modo alternativo de navegação em árvore hierárquica expansível no File Manager com cálculo e exibição recursiva de tamanho de pastas e contagem de arquivos para identificação rápida de diretórios pesados.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Baixo/Médio | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/onyxsh/filemanager/manager.py`, `src/onyxsh/filemanager/models.py`.

---

## 📅 Matriz de Versões e Entregas Sugerida

| Versão | Foco Principal | Principais Funcionalidades Previstas |
| :--- | :--- | :--- |
| **`v0.9.0`** | **Produtividade & Core UX** | • Command Palette (`Ctrl+Shift+P`)<br>• Restauração Automática de Sessões<br>• Integração Semântica OSC 133<br>• Histórico Inteligente e Snippets de Comandos<br>• Autocomplete e Notificações Desktop<br>• Novo Logo Vetorial Oficial OnyxSH |
| **`v0.10.0`** | **File Manager 2.0 & IA Avançada** | • **Quick Look (Preview com Tecla `Espaço`)**<br>• **Ações Rápidas de Terminal & IA no File Manager**<br>• **Atalhos Rápidos (Bookmarks) e Barra de Status com Espaço Livre**<br>• **Badges Visuais de Permissões (+x)**<br>• **Verificador & Comparador de Checksums / Hash**<br>• Production Guard & Roteamento Inteligente de IA<br>• Modo Estritamente Offline & Diagnóstico (`--diagnose`) |
| **`v0.11.0`** | **DevOps, Observabilidade & Remoto** | • Modo Dual-Pane Local ⇄ Remoto no File Manager<br>• Tree View Hierárquica com Métricas de Disco<br>• Dashboard de Recursos em Tempo Real (CPU/RAM/Rede)<br>• Gráficos & Imagens no Terminal (Protocolo Sixel)<br>• Health Check e Auto-Reconexão SSH<br>• Execução em Múltiplos Hosts (Multi-Host Exec)<br>• SFTP com Comparação de Diffs<br>• Auto-Correção Proativa de Erros de Terminal<br>• Exportação com Anotações & Relatório HTML de Saúde |
| **`v1.0.0`** | **Maturidade, Extensibilidade & Cofres** | • API de Plugins (Onyx Bridge)<br>• Ferramentas Customizadas para o Agente & Catálogo de Receitas<br>• Integração com Gerenciadores de Segredos (Bitwarden, 1Password)<br>• Compartilhamento Instantâneo de Snippets via Link<br>• Estabilização Completa de Pacotes Flatpak, Debian e AUR |
| **`v1.1.0`** | **Colaboração & Proteção Avançada** | • Sessões Compartilhadas & Terminal Multiplayer (Pair Programming)<br>• Modo "Sombra" (Dry-Run Sandbox com visualização de diff)<br>• Detecção Proativa de Comportamento Anômalo<br>• Aprendizado por Demonstração (Demonstration Learning) |

---

*Documento mantido e versionado no repositório OnyxSH como guia oficial de desenvolvimento.*

