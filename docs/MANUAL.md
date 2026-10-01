# Changelog

Todas as mudanças notáveis no projeto OnyxSH serão documentadas neste arquivo.

O formato é baseado no [Keep a Changelog](https://keepachangelog.com/pt-BR/1.0.0/) e este projeto segue o [Semantic Versioning](https://semver.org/lang/pt-BR/).

### Catálogo de Comandos Nativos
Inclui especificações ricas para mais de 50 utilitários essenciais:
- **Pacotes e Serviços:** `apt`, `dnf` (Fedora/RHEL, aliases `yum`/`microdnf`), `systemctl`, `journalctl`, `ufw`.

### Catálogo de Comandos Nativos (Multi-Distro)
Inclui especificações ricas para 58 utilitários essenciais com suporte multi-distro:
- **Pacotes e Serviços:** `apt`, `dnf`, `yum`, `pacman`, `systemctl`, `journalctl`, `ufw`.
- **Containers e Redes:** `docker`, `ssh`, `curl`, `ping`, `ip`, `ss`, `rsync`.
- **Arquivos e Navegação:** `tar`, `chmod`, `chown`, `find`, `grep`, `mkdir`, `rm`, `ls`, `cp`, `mv`, `cat`.
- **Monitoramento:** `htop`, `top`, `ps`, `df`, `du`, `free`, `kill`.

### Comandos do Seu Sistema (PATH)
Além do catálogo curado, o engine detecta executáveis do `PATH` da sua máquina (scripts próprios, programas instalados, atalhos) com cache de 60s. Desative com `autocomplete_system_enabled: false`.

### Modo Explorar (Ctrl + Espaço)
- Com o prompt vazio, pressione <kbd>Ctrl</kbd> + <kbd>Espaço</kbd> para listar todos os comandos possíveis.
- Com um comando digitado (ex: `dnf `), lista todos os subcomandos e flags.

### Navegação e Inserção por Teclado
- **Navegar:** Use as setas <kbd>↑</kbd> e <kbd>↓</kbd> para selecionar a opção desejada.
- **Confirmar:** Pressione <kbd>Tab</kbd> ou <kbd>Enter</kbd> para autocompletar o comando ou argumento.
- **Fechar Popup:** Pressione <kbd>Esc</kbd>.

---

## 7. Histórico Enriquecido de Comandos (`Ctrl + H`)

Pressione <kbd>Ctrl</kbd> + <kbd>H</kbd> em qualquer terminal para abrir a janela de Histórico Enriquecido.

### Busca Fuzzy e Filtros Contextuais
- Digite qualquer parte de um comando antigo, argumento ou caminho de diretório.
- **Filtros por Pílulas:**
  - **Todos:** Exibe todo o histórico unificado.
  - **📁 Diretório Atual:** Filtra apenas comandos que foram executados dentro do `$PWD` atual.
  - **🖥️ Host Remoto:** Filtra comandos executados no servidor da sessão ativa.
  - **⭐ Favoritos:** Exibe apenas comandos que você marcou como favoritos.

### Fixação de Favoritos (⭐ Pinned)
- Clique na estrela ao lado de um comando ou selecione a linha e pressione <kbd>Ctrl</kbd> + <kbd>P</kbd>.
- Comandos favoritados têm prioridade no topo da lista e **são preservados durante limpezas normais**.

### Limpeza Flexível de Histórico
- Clique no ícone de lixeira no cabeçalho ou pressione <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>Delete</kbd>:
  - **Limpar Não Favoritos:** Apaga o histórico mantendo todos os comandos com estrela ⭐.
  - **Limpar com Falha:** Apaga comandos que terminaram com erro (`exit_code != 0`).
  - **Limpar Tudo:** Limpa absolutamente todo o banco de histórico.

---

## 8. Command Palette Spotlight (`Ctrl + Shift + P`)

Inspirado nas paletas de comandos dos editores modernos (VS Code, Sublime), o Command Palette permite controlar 100% do OnyxSH via teclado:
- Pressione <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>P</kbd>.
- Digite o que deseja fazer: *"novo túnel"*, *"dividir vertical"*, *"assistente ia"*, *"exportar"*, *"preferências"*, *"limpar tela"* ou o nome de qualquer servidor SSH salvo.
- Pressione <kbd>Enter</kbd> para executar a ação imediatamente.

---

## 9. Assistente de IA Integrado e Modo Agente Seguro

Pressione <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>I</kbd> ou clique no botão de IA para abrir o painel lateral de chat.

### Provedores Suportados
No diálogo de preferências da IA (ícone de engrenagem no painel), configure seu provedor favorito:
- **Ollama / LM Studio (Local & Privativo):** Execução 100% offline na sua GPU.
- **Google Gemini:** Modelos rápidos com ampla janela de contexto.
- **Groq:** Inferência ultrarrápida em LPU (Llama 3 / Mixtral).
- **OpenRouter:** Acesso a dezenas de modelos comerciais e open source.

### Detecção Automática de GPU e VRAM
- O OnyxSH detecta sua placa de vídeo e quantidade de VRAM disponível.
- **Recomendação de Contexto:** Recomenda o tamanho de janela ideal (`num_ctx` de 4K até 128K) para evitar estouro de memória e lentidão.
- **Gerenciamento de Ciclo de Vida:** O modelo é pré-carregado em background ao abrir o terminal e descarregado automaticamente da memória GPU ao fechar o app.

### Diagnóstico de Erros em 1 Clique
Quando um comando falha no terminal (ex: `Permission denied`, `Syntax error`, `Connection refused`), um badge de erro surge ao lado do prompt. Clique em **Analisar com IA** para que a inteligência analise o comando, a mensagem de erro e proponha a solução exata.

### Modo Agente com Trilha de Auditoria e Rollback
Ao solicitar tarefas complexas de automação à IA:
1. A IA gera um plano estruturado de ações (`ActionPlan`).
2. O motor de políticas de segurança classifica cada operação em níveis (0 a 4).
3. Alterações em arquivos exibem uma visualização de *Diff* lado a lado.
4. Backups automáticos são criados antes de qualquer modificação, permitindo reversão completa (*Rollback*) a qualquer momento pelo registro de auditoria.

---

## 10. Gerenciador de Arquivos Remoto (SFTP) & Servidor TFTP

### Painel Lateral SFTP e Drag & Drop
- Em abas conectadas via SSH, clique no botão de pasta na barra de ferramentas para abrir o navegador de arquivos remoto SFTP.
- **Navegação Intuitiva:** Clique duas vezes em diretórios, visualize permissões, tamanhos e datas.
- **Upload / Download por Arraste (Drag & Drop):** Arraste arquivos do seu gerenciador de arquivos do Linux diretamente para o painel SFTP para iniciar o upload.

### Edição Remota Transparente
- Clique com o botão direito em um arquivo remoto e escolha **Editar Arquivo**.
- O OnyxSH baixa o arquivo para um cache temporário seguro e o abre no seu editor de texto local padrão (ex: Gedit, VS Code, Kate).
- Ao salvar o arquivo no seu editor, o OnyxSH detecta a alteração e faz o upload automático de volta para o servidor remoto.

### Servidor TFTP Integrado
Para administradores de rede que trabalham com switches, roteadores e equipamentos embarcados:
- Acesse Menu Principal ➔ **Servidor TFTP**.
- Inicie um servidor TFTP local na porta configurada para enviar e receber firmwares e arquivos de configuração (`running-config`).

---

## 11. Exportação Multi-formato do Terminal

Acesse Menu Principal ➔ **Exportar Terminal...** ou clique no botão de exportação na barra de busca:
1. Escolha o escopo: **Buffer Completo** ou apenas o **Texto Selecionado**.
2. Selecione o formato desejado:
   - 📄 **Texto Puro (`.txt`):** Texto simples e limpo.
   - 📋 **Arquivo de Log (`.log`):** Inclui cabeçalho completo com nome da sessão, host, diretório `$PWD`, data/hora e dimensões do terminal.
   - 📝 **Markdown (`.md`):** Formatado em blocos de código markdown prontos para documentação no GitHub/GitLab.
   - 🌐 **HTML Estilizado (`.html`):** Página web independente com tema escuro elegante e preservação fiel de cores ANSI.
   - 🎬 **Asciinema (`.cast`):** Formato padrão para reprodução de sessões no player Asciinema.

---

## 12. Busca Avançada no Scrollback (`Ctrl + Shift + F`)

Pressione <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F</kbd> para abrir a barra de busca flutuante no terminal:
- **Botões Flat Modernos:**
  - **`Aa`**: Diferenciar maiúsculas de minúsculas (*Case Sensitive*).
  - **`\b`**: Buscar apenas palavras inteiras (*Whole Word*).
  - **`.*`**: Ativar modo de Expressões Regulares (*Regex*).
- **Navegação por Teclado:**
  - <kbd>Enter</kbd>: Pular para a próxima correspondência.
  - <kbd>Shift</kbd> + <kbd>Enter</kbd>: Voltar para a correspondência anterior.
  - <kbd>Esc</kbd>: Fechar a barra de busca.
- **Contador em Tempo Real:** Exibe a contagem exata (ex: `3 de 42 correspondências`).

---

## 13. Rastreamento Semântico de Shell (OSC 133)

O OnyxSH implementa nativamente as sequências de escape do padrão **OSC 133** (Semantic Shell Integration):
- **Tempo de Execução Preciso:** Cada comando executado mede o tempo exato com precisão de milissegundos (ex: `⏱ 2.34s`).
- **Navegação Rápida entre Prompts:** Pressione <kbd>Alt</kbd> + <kbd>↑</kbd> para rolar a tela diretamente para o início do prompt anterior, ou <kbd>Alt</kbd> + <kbd>↓</kbd> para avançar para o próximo prompt.
- **Isolamento de Saída:** Permite copiar exclusivamente a saída de um comando específico sem arrastar o prompt ou comandos vizinhos.

---

## 14. Tabela Completa de Atalhos de Teclado

| Atalho | Ação | Contexto |
|---|---|---|
| <kbd>F2</kbd> | Abrir Diálogo de Preferências | Geral |
| <kbd>Ctrl</kbd> + <kbd>H</kbd> | Abrir Histórico Enriquecido de Comandos | Terminal |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>P</kbd> | Abrir Command Palette (Spotlight) | Geral |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>I</kbd> | Abrir/Fechar Painel do Assistente de IA | Geral |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F</kbd> | Abrir Busca no Terminal | Terminal |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>B</kbd> | Alternar Barra de Transmissão (Broadcast) | Geral |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>T</kbd> | Nova Aba de Terminal Local | Abas |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>W</kbd> | Fechar Aba ou Painel Dividido Ativo | Abas |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>D</kbd> | Dividir Terminal Horizontalmente | Painéis |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>E</kbd> | Dividir Terminal Verticalmente | Painéis |
| <kbd>Alt</kbd> + <kbd>↑</kbd> | Pular para o Prompt Anterior (OSC 133) | Terminal |
| <kbd>Alt</kbd> + <kbd>↓</kbd> | Pular para o Próximo Prompt (OSC 133) | Terminal |
| <kbd>Ctrl</kbd> + <kbd>Espaço</kbd> | Explorar comandos/subcomandos (autocomplete browse) | Terminal |
| <kbd>Ctrl</kbd> + <kbd>+</kbd> | Aumentar Tamanho da Fonte (Zoom In) | Terminal |
| <kbd>Ctrl</kbd> + <kbd>-</kbd> | Diminuir Tamanho da Fonte (Zoom Out) | Terminal |
| <kbd>Ctrl</kbd> + <kbd>0</kbd> | Restaurar Tamanho Padrão da Fonte | Terminal |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>C</kbd> | Copiar Texto Selecionado | Terminal |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd> | Colar da Área de Transferência | Terminal |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>Del</kbd> | Abrir Diálogo de Limpeza de Histórico | Janela Histórico |

---

## 15. Configurações e Armazenamento Local

Todos os dados e configurações do OnyxSH ficam salvos no seu diretório de usuário em `~/.config/onyxsh/`:

```text
~/.config/onyxsh/
├── settings.json          # Preferências de interface, fontes, temas, atalhos e IA
├── sessions.json          # Árvore de sessões SSH, pastas e configurações salvas
├── command_history.db     # Banco de dados SQLite do histórico enriquecido
├── session_state.json     # Estado das abas para restauração automática de sessão
├── layouts/               # Modelos salvos de divisão de painéis e telas
└── backups/               # Backups de arquivos de configuração e sessões
```

> [!TIP]
> Para fazer backup de todas as suas sessões e configurações do OnyxSH, basta copiar a pasta `~/.config/onyxsh/`.
