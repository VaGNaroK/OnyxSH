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
      ┌─────────────────┬──────────────┴───────┬─────────────────┐
      │                 │                      │                 │
┌─────▼──────────┐ ┌────▼──────────┐ ┌─────────▼────────┐ ┌──────▼──────────┐
│  1. Core UX &  │ │  2. Onyx      │ │ 3. IA Avançada   │ │  4. Qualidade & │
│  Produtividade │ │  Guard (Sec)  │ │ & Onyx Bridge    │ │  Confiabilidade │
└────────────────┘ └───────────────┘ └──────────────────┘ └─────────────────┘
```

---

## 1. Experiência de Terminal & Produtividade (Core UX)

### 📌 1.1. Command Palette (`Ctrl + Shift + P`)
- [x] **Descrição:** Interface de busca fuzzy centralizada (estilo VS Code / Sublime Text) para invocar qualquer ação do terminal sem navegar por menus.
- [x] **Ações Disponíveis:** Nova aba, divisão horizontal/vertical, alternar temas, abrir configurações, conectar em sessões SSH, abrir SFTP, exportar conversas, limpar histórico, abrir log de auditoria.
- [x] **Status:** ✅ Implementado na versão `0.8.20` (`src/zashterminal/ui/dialogs/command_palette_dialog.py`).
- [x] **Prioridade:** 🔴 Alta | **Esforço:** Médio | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/zashterminal/ui/dialogs/command_palette_dialog.py`, `src/zashterminal/ui/actions.py`, `src/zashterminal/app.py`, `src/zashterminal/settings/config.py`.


### 📌 1.2. Restauração Automática e Inteligente de Sessões (Session Restore)
- [x] **Descrição:** Salvar e restaurar o estado completo da aplicação entre fechamentos e reinicializações.
- [x] **Estado Restaurado:** Abas ativas, layouts em split, diretórios correntes (`$PWD`), sessões SSH abertas (com opção de auto-reconexão), histórico e visibilidade do painel de IA/barras laterais.
- [x] **Modos de Operação nas Preferências:**
  - `Sempre restaurar sessão anterior` (padrão com foco e estado completo).
  - `Perguntar ao iniciar` (toast não-intrusivo de restauração com 1 clique).
  - `Nunca restaurar` (inicia limpo no `$HOME`).
  - Switches de configuração: Auto-reconexão SSH e restauração de painéis da UI.
- [x] **Status:** ✅ Implementado no ciclo `v0.9.0` (`src/zashterminal/state/window_state.py`, `src/zashterminal/terminal/tabs.py`, `src/zashterminal/window.py`, `src/zashterminal/ui/dialogs/preferences_dialog.py`).
- [x] **Prioridade:** 🔴 Alta | **Esforço:** Alto | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/zashterminal/state/window_state.py`, `src/zashterminal/terminal/tabs.py`, `src/zashterminal/window.py`, `src/zashterminal/ui/actions.py`, `src/zashterminal/ui/dialogs/preferences_dialog.py`.

### 📌 1.3. Integração Semântica com Shell (OSC 133 / Semantic Prompts)
- [x] **Descrição:** Suporte a sequências de controle OSC 133 (FinalTerm / iTerm2 semantics) para marcação inteligente de prompts, comandos e saídas do shell.
- [x] **Funcionalidades Habilitadas:**
  - Navegação entre comandos: pular para o prompt anterior (`Alt + Up`) / próximo (`Alt + Down`).
  - Seleção cirúrgica de saída: copiar apenas a saída do último comando executado.
  - Indicador visual de status de saída (ícone de sucesso `0` ou erro `>0` na barra do painel).
  - Medição de tempo de execução por comando (ex: `⏱ 1.4s`).
  - Botão de envio rápido da saída de um comando específico para análise no chat de IA.
- [x] **Status:** ✅ Implementado no ciclo `v0.9.0` (`src/zashterminal/terminal/semantic_tracker.py`, `src/zashterminal/terminal/spawner.py`, `src/zashterminal/terminal/manager.py`, `src/zashterminal/terminal/tabs.py`, `src/zashterminal/ui/actions.py`, `src/zashterminal/ui/dialogs/command_palette_dialog.py`).
- [x] **Prioridade:** 🟡 Média/Alta | **Esforço:** Médio/Alto | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/zashterminal/terminal/semantic_tracker.py`, `src/zashterminal/terminal/spawner.py`, `src/zashterminal/terminal/manager.py`, `src/zashterminal/terminal/tabs.py`, `src/zashterminal/ui/actions.py`, `src/zashterminal/ui/dialogs/command_palette_dialog.py`.

### 📌 1.4. Histórico Enriquecido de Comandos com Busca Fuzzy
- [x] **Descrição:** Persistência estruturada em banco SQLite com metadados detalhados de cada comando executado.
- [x] **Campos:** `command`, `cwd`, `host`, `session_name`, `exit_code`, `duration_ms`, `timestamp`, `is_pinned`, `execution_count`, `last_executed`.
- [x] **Recursos:** Busca fuzzy com ranqueamento, filtros em tempo real (Todos, Diretório Atual `$PWD`, Este Host/Sessão SSH, Favoritos ⭐), inserção no prompt (`Tab`), execução imediata (`Enter`), atalho global `Ctrl + R` e Command Palette.
- [x] **Status:** ✅ Implementado no ciclo `v0.9.0` (`src/zashterminal/data/command_history_manager.py`, `src/zashterminal/ui/dialogs/command_history_dialog.py`, `src/zashterminal/ui/actions.py`, `src/zashterminal/window.py`, `src/zashterminal/terminal/manager.py`).
- [x] **Prioridade:** 🟡 Média | **Esforço:** Médio | **Alvo:** `v0.9.0`
- [x] **Módulos Afetados:** `src/zashterminal/data/command_history_manager.py`, `src/zashterminal/ui/dialogs/command_history_dialog.py`, `src/zashterminal/ui/actions.py`, `src/zashterminal/window.py`, `src/zashterminal/terminal/manager.py`.

### 📌 1.5. Gerenciador de Snippets de Comandos Reutilizáveis
- [ ] **Descrição:** Criação e execução de templates de comandos parametrizáveis com substituição de variáveis.
- [ ] **Exemplo:** `docker logs -f {{container_name}} --tail {{lines}}` ou `rsync -avz {{src}} {{user}}@{{host}}:{{dest}}`.
- [ ] **Variáveis Nativas:** `{{cwd}}`, `{{host}}`, `{{user}}`, `{{date}}`, `{{branch_git}}`.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Baixo/Médio | **Alvo:** `v0.9.0`
- [ ] **Módulos Afetados:** `src/zashterminal/ui/dialogs/command_manager_dialog.py`.

### 📌 1.6. Notificações Desktop para Comandos Longos
- [ ] **Descrição:** Notificação nativa via portal XDG/D-Bus (`org.freedesktop.Notifications`) quando um comando em segundo plano ou em aba inativa terminar após um tempo limite configurável (ex: > 10s).
- [ ] **Informações:** Nome do comando, tempo de execução e código de retorno (sucesso ou falha).
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Baixo | **Alvo:** `v0.9.0`
- [ ] **Módulos Afetados:** `src/zashterminal/terminal/manager.py`.

### 📌 1.7. Busca Avançada no Scrollback e Exportação do Terminal
- [ ] **Descrição:** Aprimorar o painel de busca no buffer do terminal com suporte a Regex, correspondência de maiúsculas/minúsculas e exportação direta do buffer/seleção em múltiplos formatos (`.txt`, `.log`, `.md`, `.html` e `.asciinema`).
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Baixo/Médio | **Alvo:** `v0.9.0`
- [ ] **Módulos Afetados:** `src/zashterminal/ui/window_ui.py`.

---

## 2. Segurança, DevOps & Infraestrutura ("Zash Guard & Ops")

### 🛡️ 2.1. Modo Proteção de Produção (Production Guard)
- [ ] **Descrição:** Modo de segurança reforçada ativado automaticamente ao conectar em hosts/sessões marcadas como `Produção`.
- [ ] **Barreiras de Segurança:**
  - Banner visual permanente vermelho/laranja no topo do terminal indicando `AMBINETE DE PRODUÇÃO`.
  - Bloqueio estrito de comandos de alto risco (`rm -rf`, `mkfs`, `dd`, `systemctl disable --now`, `shutdown`).
  - Exigência de confirmação dupla com digitação do nome do host antes de executar ações de risco nível 2 ou 3.
  - Bloqueio automático de envio de saídas confidenciais de produção para APIs de IA externas.
- [ ] **Prioridade:** 🔴 Muito Alta | **Esforço:** Médio | **Alvo:** `v0.10.0`
- [ ] **Módulos Afetados:** `src/zashterminal/agent/policy_engine.py`, `src/zashterminal/sessions/manager.py`.

### 🛡️ 2.2. Gerenciador Visual de Túneis SSH e Port Forwarding
- [ ] **Descrição:** Interface gráfica para criar, monitorar e alternar túneis SSH de forma visual.
- [ ] **Modos:**
  - `Local Forwarding` (ex: acessar banco de dados remoto em `localhost:5432`).
  - `Remote Forwarding` (ex: expor porta local para o servidor remoto).
  - `Dynamic Forwarding (SOCKS5 Proxy)` para navegação segura através do túnel SSH.
- [ ] **Recursos:** Indicador de status (conectado / erro), auto-reconexão e ativação automática ao conectar na sessão.
- [ ] **Prioridade:** 🟡 Alta | **Esforço:** Médio/Alto | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/zashterminal/sessions/`, `src/zashterminal/ui/dialogs/`.

### 🛡️ 2.3. Health Check e Auto-Reconexão de Sessões SSH
- [ ] **Descrição:** Monitoramento proativo da saúde das conexões remotas.
- [ ] **Recursos:** Detecção imediata de quebra de socket SSH com exibição de banner de aviso e tentativa automática de reconexão (`KeepAlive` inteligente); medição de latência (ping/RTT em ms) exibida na árvore de sessões.
- [ ] **Prioridade:** 🟡 Alta | **Esforço:** Médio | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/zashterminal/sessions/`, `src/zashterminal/ui/sidebar_manager.py`.

### 🛡️ 2.4. Execução em Múltiplos Hosts (Multi-Host Exec / Cluster Commands)
- [ ] **Descrição:** Capacidade de selecionar múltiplos servidores na árvore de sessões e disparar um comando em paralelo.
- [ ] **Recursos:** Saída agrupada por host, visualização de status de sucesso/falha individual, auditoria completa da operação.
- [ ] **Prioridade:** 🟢 Média/Alta | **Esforço:** Alto | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/zashterminal/sessions/`, `src/zashterminal/ui/dialogs/`.

### 🛡️ 2.5. SFTP com Comparação de Diffs Remotos
- [ ] **Descrição:** Integrar o visualizador de diffs do Zashterminal com o cliente SFTP.
- [ ] **Recursos:** Comparar arquivo local com versão remota antes de enviar (`Upload Diff`); comparar versões remotas antes de sobrescrever; backup automático remoto antes da sobrescrita.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Médio | **Alvo:** `v0.11.0`
- [ ] **Módulos Afetados:** `src/zashterminal/filemanager/`, `src/zashterminal/ui/dialogs/diff_review_dialog.py`.

### 🛡️ 2.6. Encadeamento Criptográfico nos Logs de Auditoria (Hash Chain)
- [ ] **Descrição:** Tornar os logs de auditoria à prova de adulteração adicionando SHA-256 encadeado (`previous_hash` + `current_event_hash`) em cada registro de `audit_log.json`.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Baixo/Médio | **Alvo:** `v0.10.0`
- [ ] **Módulos Afetados:** `src/zashterminal/agent/audit_logger.py`.

---

## 3. IA Avançada, Agente Autônomo & Extensibilidade ("Zash Agent & Bridge")

### 🤖 3.1. Modo Interativo "Plan before Execute"
- [ ] **Descrição:** Quando o agente receber uma tarefa complexa de múltiplos passos, gerar primeiro uma árvore de plano visual no chat antes de qualquer execução.
- [ ] **Recursos:**
  - Detalhamento de cada etapa com indicador de risco, comando e arquivo afetado.
  - **Aprovação Granular por Etapas:** Usuário pode aprovar tudo de uma vez, aprovar apenas leituras, aprovar escritas com revisão de diff ou revisar passo a passo.
- [ ] **Prioridade:** 🔴 Muito Alta | **Esforço:** Alto | **Alvo:** `v0.10.0`
- [ ] **Módulos Afetados:** `src/zashterminal/agent/context_manager.py`, `src/zashterminal/ui/widgets/ai_chat_panel.py`.

### 🤖 3.2. Verificação Pós-Execução Automatizada (Post-Verification Loop)
- [ ] **Descrição:** Após o agente executar um comando que altera o sistema (ex: reiniciar um serviço ou aplicar uma configuração), ele executa verificações automáticas de validação.
- [ ] **Exemplo:** Após `sudo systemctl restart nginx`, checar automaticamente `systemctl is-active nginx` e exibir os últimos logs do `journalctl` se houver falha.
- [ ] **Prioridade:** 🟡 Alta | **Esforço:** Médio | **Alvo:** `v0.10.0`
- [ ] **Módulos Afetados:** `src/zashterminal/agent/`.

### 🤖 3.3. Roteamento Inteligente de Provedores de IA (Smart Model Routing)
- [ ] **Descrição:** Permitir associar diferentes modelos de IA conforme a complexidade da tarefa.
- [ ] **Perfis:**
  - *Perguntas simples / sintaxe de comandos:* Modelo rápido / local (Groq / Ollama).
  - *Planejamento complexo / scripts longos:* Modelo avançado (Gemini 2.5 Flash / Claude 3.5 Sonnet).
  - *Análise de segurança / auditoria:* Modelo forte com raciocínio profundo.
- [ ] **Prioridade:** 🟡 Média/Alta | **Esforço:** Médio | **Alvo:** `v0.10.0`
- [ ] **Módulos Afetados:** `src/zashterminal/terminal/ai_assistant.py`, `src/zashterminal/settings/manager.py`.

### 🤖 3.4. Modo Estritamente Offline / Local-Only com Indicador Visual
- [ ] **Descrição:** Chave global de privacidade que desativa qualquer saída para provedores externos de IA (Gemini, Groq, OpenRouter), forçando o uso exclusivo de modelos locais via Ollama/LocalAI e exibindo um selo visual `MODO OFFLINE ATIVO`.
- [ ] **Prioridade:** 🟡 Alta | **Esforço:** Baixo/Médio | **Alvo:** `v0.10.0`
- [ ] **Módulos Afetados:** `src/zashterminal/ui/dialogs/ai_config_dialog.py`, `src/zashterminal/terminal/ai_assistant.py`.

### 🤖 3.5. Integrações Específicas com Git
- [ ] **Descrição:** Assistente especializado em fluxos de trabalho com Git.
- [ ] **Recursos:** Gerador de mensagens de commit baseadas no `git diff --staged` (padrão Conventional Commits), detecção de segredos esquecidos antes do commit, explicação e resolução guiada de conflitos de merge.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Médio | **Alvo:** `v0.10.0`
- [ ] **Módulos Afetados:** `src/zashterminal/agent/tools/`.

### 🔌 3.6. API de Extensibilidade e Plugins ("Zash Bridge")
- [ ] **Descrição:** Framework de extensões em Python que permite à comunidade criar plugins modulares.
- [ ] **Capacidades dos Plugins:**
  - Registrar novas ferramentas seguras para o agente de IA (`AgentTool`).
  - Adicionar itens na Command Palette.
  - Criar novos painéis laterais e conectores de nuvem (AWS, Docker, Kubernetes).
  - Adicionar temas e regras de sintaxe personalizadas.
- [ ] **Prioridade:** 🟡 Média/Alta | **Esforço:** Alto | **Alvo:** `v1.0.0`
- [ ] **Módulos Afetados:** `src/zashterminal/plugins/` (novo pacote).

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

### 🧪 4.2. Modo Diagnóstico Seguro (`zashterminal --diagnose`)
- [ ] **Descrição:** Comando CLI que gera um relatório técnico sanitizado do sistema (distro, Wayland/X11, versão do GTK/VTE, GPU, runtime Python, permissões Flatpak e logs recentes sem dados pessoais) para agilizar suporte e abertura de issues no GitHub.
- [ ] **Prioridade:** 🟢 Média | **Esforço:** Baixo | **Alvo:** `v0.9.0`
- [ ] **Módulos Afetados:** `src/zashterminal/app.py`, `src/zashterminal/utils/platform.py`.

---

## 📅 Matriz de Versões e Entregas Sugerida

| Versão | Foco Principal | Principais Funcionalidades Previstas |
| :--- | :--- | :--- |
| **`v0.9.0`** | **Produtividade & Core UX** | • Command Palette (`Ctrl+Shift+P`)<br>• Restauração Automática de Sessões<br>• Integração Semântica OSC 133<br>• Histórico Inteligente e Snippets de Comandos<br>• Modo Diagnóstico (`--diagnose`) |
| **`v0.10.0`** | **Segurança & Agente Avançado** | • Production Guard (Modo Produção)<br>• Modo "Plan before Execute" com aprovação granular<br>• Verificação Pós-Execução Automatizada<br>• Roteamento Inteligente de Modelos<br>• Modo Estritamente Offline<br>• Encadeamento Criptográfico de Auditoria |
| **`v0.11.0`** | **DevOps & Operações Remotas** | • Gerenciador Visual de Túneis SSH<br>• Health Check e Auto-Reconexão SSH<br>• Execução em Múltiplos Hosts (Multi-Host Exec)<br>• SFTP com Comparação de Diffs<br>• Integração Básica com Docker/Podman |
| **`v1.0.0`** | **Maturidade & Extensibilidade** | • API de Plugins (Zash Bridge)<br>• Ferramentas Customizadas para o Agente<br>• Estabilização Completa de Pacotes Flatpak, Debian e AUR<br>• Documentação e Threat Model Final |

---

*Documento mantido e versionado no repositório Zashterminal como guia oficial de desenvolvimento.*
