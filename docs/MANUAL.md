<p align="right">
  <strong>🇧🇷 Português</strong> | <a href="MANUAL.en.md">🇺🇸 English</a>
</p>

# 📖 Manual Completo do Usuário — OnyxSH

<p align="center">
  <img src="../usr/share/icons/hicolor/scalable/apps/onyxsh.svg" alt="Logo OnyxSH" width="96" height="96">
</p>

Bem-vindo ao **Manual do Usuário do OnyxSH**. Este guia foi elaborado para apresentar todas as funcionalidades, fluxos de trabalho, atalhos de teclado e configurações do terminal, desde os conceitos básicos até os recursos avançados para engenheiros de DevOps, SREs e administradores de sistemas Linux.

---

## 📑 Índice

1. [Visão Geral e Arquitetura](#1-visão-geral-e-arquitetura)
2. [Interface Gráfica e Navegação](#2-interface-gráfica-e-navegação)
   - [Abas e Título Dinâmico](#abas-e-título-dinâmico)
   - [Divisão de Painéis (Splits Horizontais e Verticais)](#divisão-de-painéis-splits)
   - [Transmissão em Massa (Broadcast Mode)](#transmissão-em-massa-broadcast-mode)
3. [Gerenciador de Sessões SSH & SFTP](#3-gerenciador-de-sessões-ssh--sftp)
   - [Criando Pastas e Conexões](#criando-pastas-e-conexões)
   - [Autenticação por Chaves SSH e Senhas Seguras](#autenticação-por-chaves-ssh-e-senhas-seguras)
   - [Gateways Corporativos PAM / Balabit One Identity](#gateways-corporativos-pam--balabit)
   - [Importação de Sessões do SecureCRT](#importação-de-sessões-do-securecrt)
4. [Gerenciador Visual de Túneis SSH & Port Forwarding](#4-gerenciador-visual-de-túneis-ssh--port-forwarding)
   - [Local Port Forwarding (-L)](#local-port-forwarding--l)
   - [Remote Port Forwarding (-R)](#remote-port-forwarding--r)
   - [Dynamic SOCKS5 Proxy (-D)](#dynamic-socks5-proxy--d)
   - [Controle em Tempo Real e Auto-start](#controle-em-tempo-real-e-auto-start)
5. [Modo Proteção de Produção (Production Guard)](#5-modo-proteção-de-produção-production-guard)
   - [Definição de Ambientes de Produção](#definição-de-ambientes-de-produção)
   - [Banner Visual de Alta Visibilidade](#banner-visual-de-alta-visibilidade)
   - [Interceptação de Comandos Destrutivos](#interceptação-de-comandos-destrutivos)
   - [Confirmação Dupla Segura e Ofuscação de Segredos](#confirmação-dupla-segura-e-ofuscação-de-segredos)
6. [Autocomplete Inteligente com Catálogo de Specs Linux](#6-autocomplete-inteligente-com-catálogo-de-specs-linux)
   - [Popup Flutuante Ancorado ao Cursor](#popup-flutuante-ancorado-ao-cursor)
   - [Catálogo de Comandos Nativos](#catálogo-de-comandos-nativos)
   - [Navegação e Inserção por Teclado](#navegação-e-inserção-por-teclado)
7. [Histórico Enriquecido de Comandos (`Ctrl + H`)](#7-histórico-enriquecido-de-comandos-ctrl--h)
   - [Busca Fuzzy e Filtros Contextuais](#busca-fuzzy-e-filtros-contextuais)
   - [Fixação de Favoritos (⭐ Pinned)](#fixação-de-favoritos--pinned)
   - [Limpeza Flexível de Histórico](#limpeza-flexível-de-histórico)
8. [Command Palette Spotlight (`Ctrl + Shift + P`)](#8-command-palette-spotlight-ctrl--shift--p)
9. [Assistente de IA Integrado e Modo Agente Seguro](#9-assistente-de-ia-integrado-e-modo-agente-seguro)
   - [Provedores Suportados (Ollama, Gemini, Groq, OpenRouter)](#provedores-suportados)
   - [Detecção Automática de GPU e VRAM](#detecção-automática-de-gpu-e-vram)
   - [Diagnóstico de Erros em 1 Clique](#diagnóstico-de-erros-em-1-clique)
   - [Modo Agente com Trilha de Auditoria e Rollback](#modo-agente-com-trilha-de-auditoria-e-rollback)
10. [Gerenciador de Arquivos Remoto (SFTP) & Servidor TFTP](#10-gerenciador-de-arquivos-remoto-sftp--servidor-tftp)
    - [Painel Lateral SFTP e Drag & Drop](#painel-lateral-sftp-e-drag--drop)
    - [Edição Remota Transparente](#edição-remota-transparente)
    - [Servidor TFTP Integrado](#servidor-tftp-integrado)
11. [Exportação Multi-formato do Terminal](#11-exportação-multi-formato-do-terminal)
12. [Busca Avançada no Scrollback (`Ctrl + Shift + F`)](#12-busca-avançada-no-scrollback-ctrl--shift--f)
13. [Rastreamento Semântico de Shell (OSC 133)](#13-rastreamento-semântico-de-shell-osc-133)
14. [Tabela Completa de Atalhos de Teclado](#14-tabela-completa-de-atalhos-de-teclado)
15. [Configurações e Armazenamento Local](#15-configurações-e-armazenamento-local)

---

## 1. Visão Geral e Arquitetura

O **OnyxSH** foi projetado para ser o terminal de desenvolvimento e administração de servidores definitivo no Linux. Ele combina os melhores componentes do ecossistema GNOME moderno:
- **Interface Libadwaita & GTK4:** Design moderno, fluido, adaptável e compatível com as diretrizes do GNOME.
- **Motor VTE com PTY Proxy:** Terminal de alta velocidade com renderização aceleração e isolamento seguro.
- **Banco de Dados SQLite Local:** Armazenamento transacional do histórico de comandos e metadados de execução.
- **Agente Seguro com Políticas:** Isolamento para chamadas de IA sem execução arbitrária de código sem consentimento.

---

## 2. Interface Gráfica e Navegação

### Abas e Título Dinâmico
- O OnyxSH organiza seus terminais em abas superiores.
- Quando apenas uma aba está aberta, a barra superior exibe o título limpo **OnyxSH** ou `OnyxSH - [Host da Sessão]`. Ao abrir múltiplas abas, a barra transforma-se automaticamente em uma barra de abas navegável e rolável com a roda do mouse.
- **Nova Aba:** Pressione <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>T</kbd> ou clique no botão `+`.
- **Fechar Aba:** Pressione <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>W</kbd>.

### Divisão de Painéis (Splits)
Você pode dividir qualquer aba em múltiplos terminais lado a lado ou empilhados:
- **Dividir Horizontalmente:** Pressione <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>D</kbd>.
- **Dividir Verticalmente:** Pressione <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>E</kbd>.
- **Navegar entre Painéis:** Clique com o mouse no painel desejado ou use os atalhos de foco.
- **Redimensionar:** Arraste as barras separadoras (*paned handles*) entre os terminais.

### Transmissão em Massa (Broadcast Mode)
Pressione <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>B</kbd> para ativar a barra de transmissão simultânea. Tudo o que for digitado na barra de broadcast será enviado instantaneamente para **todas as abas e painéis abertos** — ideal para atualizações em lote de múltiplos servidores.

---

## 3. Gerenciador de Sessões SSH & SFTP

Abra a barra lateral de sessões clicando no botão de menu lateral ou pelo menu principal.

### Criando Pastas e Conexões
- **Organização Hierárquica:** Crie pastas para agrupar servidores por clientes, ambientes (*Produção*, *Staging*, *Dev*) ou data centers.
- **Configuração da Sessão:**
  - Host / IP e Porta (padrão: 22).
  - Usuário (`root`, `ubuntu`, `admin`, etc.).
  - Método de Autenticação (Senha, Chave Privada SSH ou Agente SSH).
  - Codificação de Caracteres (UTF-8, ISO-8859-1, etc.).
  - Cores e Realces personalizados por sessão.

### Autenticação por Chaves SSH e Senhas Seguras
- As senhas salvas são armazenadas de forma segura e criptografada pelo sistema.
- Suporte a chaves RSA, Ed25519, ECDSA com seleção direta de arquivo `~/.ssh/id_*`.

### Gateways Corporativos PAM / Balabit
O OnyxSH detecta automaticamente banners e fluxos de autenticação interativa de gateways como **Balabit / One Identity Safeguard / CyberArk**. Ao detectar o banner, um diálogo dedicado permite informar as credenciais sem quebrar a sequência interativa do shell.

### Importação de Sessões do SecureCRT
Se você está migrando do SecureCRT:
1. Abra o Menu Principal (`☰`) ➔ **Importar Sessões do SecureCRT**.
2. Selecione o diretório de sessões do SecureCRT contendo arquivos `.ini`.
3. Toda a árvore de pastas, hosts, portas, usuários e senhas criptografadas no formato Password V2 (`02:<hex>`) será convertida e importada automaticamente.

---

## 4. Gerenciador Visual de Túneis SSH & Port Forwarding

Acesse através do Menu Principal (`☰`) ➔ **Gerenciador de Túneis SSH** ou pela Command Palette (<kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>P</kbd> ➔ *Túneis SSH*).

### Local Port Forwarding (`-L`)
Redireciona uma porta da sua máquina local para um serviço na rede do servidor remoto.
- **Exemplo:** Acessar um banco MySQL remoto na porta 3306 usando `localhost:3306` na sua máquina.
- **Configuração:** Porta Local (`3306`) ➔ Host Remoto (`127.0.0.1`) ➔ Porta Remota (`3306`).

### Remote Port Forwarding (`-R`)
Expõe uma porta de um serviço da sua máquina local diretamente no servidor remoto.
- **Exemplo:** Permitir que o servidor remoto acesse sua API de desenvolvimento local rodando na porta `8000`.

### Dynamic SOCKS5 Proxy (`-D`)
Cria um servidor proxy SOCKS5 local criptografado.
- **Exemplo:** Definir a porta local `1080`. Ao configurar navegadores ou ferramentas de rede para usar `socks5://127.0.0.1:1080`, todo o tráfego sairá com o IP do servidor SSH remoto.

### Controle em Tempo Real e Auto-start
- **Switches Rápidos:** Ligue ou desligue qualquer túnel com um simples clique no switch `Gtk.Switch`.
- **Auto-start:** Marque a opção *Iniciar automaticamente com a sessão* para que o túnel suba no momento em que você se conectar ao servidor.
- **Parada Global:** Clique no botão de parada geral para encerrar todos os processos de encaminhamento de uma só vez.

---

## 5. Modo Proteção de Produção (Production Guard)

O **Production Guard** é um sistema de segurança multicamadas projetado para prevenir acidentes humanos catastróficos em servidores críticos.

### Definição de Ambientes de Produção
- No editor de sessões ou pastas, ative a opção **Ambiente de Produção (Production)**.
- Qualquer sessão dentro de uma pasta marcada herdará automaticamente o status de produção.

### Banner Visual de Alta Visibilidade
Ao abrir um terminal de produção, uma barra superior em degradê crimson brilhante com o ícone `🛡️ PRODUÇÃO` permanece visível, deixando evidente que qualquer comando executado terá impacto real.

### Interceptação de Comandos Destrutivos
Ao digitar comandos de alto risco e pressionar <kbd>Enter</kbd>, o OnyxSH bloqueia o envio imediato ao shell e analisa o comando:
- **Exclusão em Massa:** `rm -rf`, `rm -fr`, `shred -u`, `wipefs`.
- **Formatação de Disco:** `mkfs.*`, `dd of=/dev/...`, `fdisk`, `parted`.
- **Parada de Sistema:** `shutdown`, `reboot`, `poweroff`, `init 0`.
- **Serviços Críticos:** `systemctl stop/disable`, `service ... stop`.
- **Banco de Dados:** `DROP DATABASE`, `TRUNCATE TABLE`.
- **Git Forçado:** `git push --force`, `git reset --hard`.

### Confirmação Dupla Segura e Ofuscação de Segredos
- O diálogo **Confirmação de Execução em Produção** exige que você digite o **nome exato do host ou sessão** para confirmar a ação.
- Pressione <kbd>Esc</kbd> ou clique em *Abortar* para cancelar com segurança (enviando sinal `Ctrl+C` ao terminal).
- **Proteção no Assistente de IA:** Ao analisar terminais de produção com a IA, senhas, tokens de API e certificados são automaticamente mascarados (`[REDACTED]`).

---

## 6. Autocomplete Inteligente com Catálogo de Specs Linux

O OnyxSH inclui um sistema de sugestão preditiva inteligente que opera em tempo real enquanto você digita no terminal.

### Popup Flutuante Ancorado ao Cursor
- Conforme você digita no prompt, um popup flutuante moderno aparece diretamente abaixo da posição atual do cursor.
- Apresenta ícones identificadores, sintaxe, flags e descrições claras em português.

### Catálogo de Comandos Nativos
Inclui especificações ricas para mais de 50 utilitários essenciais:
- **Pacotes e Serviços:** `apt`, `systemctl`, `journalctl`, `ufw`.
- **Containers e Redes:** `docker`, `ssh`, `curl`, `ping`, `ip`, `ss`, `rsync`.
- **Arquivos e Navegação:** `tar`, `chmod`, `chown`, `find`, `grep`, `mkdir`, `rm`, `ls`, `cp`, `mv`, `cat`.
- **Monitoramento:** `htop`, `top`, `ps`, `df`, `du`, `free`, `kill`.

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
