# 🎬 Roteiro de Apresentação e Demonstração em Vídeo — OnyxSH

- **Público-alvo:** Desenvolvedores Linux, DevOps, Engenheiros SRE, Administradores de Sistemas e entusiastas de Open Source.
- **Duração Estimada:** 6 a 8 minutos.
- **Objetivo:** Demonstrar visualmente os principais diferenciais do OnyxSH frente aos emuladores de terminal tradicionais, destacando segurança, usabilidade, IA e produtividade.

---

## 📑 Visão Geral da Estrutura

1. **[00:00 - 00:45]** — Gancho Inicial (*Hook*) & Apresentação
2. **[00:45 - 01:45]** — Interface GTK4/Libadwaita, Abas, Splits e Modo Broadcast
3. **[01:45 - 03:00]** — Gestor de Sessões SSH, Túneis Visuais e Migração do SecureCRT
4. **[03:00 - 04:00]** — Production Guard (Proteção Ativa de Servidores Críticos)
5. **[04:00 - 05:00]** — Autocomplete com Specs Linux e Histórico SQLite Enriquecido
6. **[05:00 - 06:15]** — IA Integrada (Local/GPU & Nuvem), Diagnóstico de Erros e Modo Agente
7. **[06:15 - 07:15]** — SFTP com Drag & Drop, Edição Remota e Caderno de Bordo (Runbook)
8. **[07:15 - 08:00]** — Como Instalar (Flatpak / .deb / install.sh) e Conclusão (CTA)

---

## 🎥 Roteiro Detalhado (Cena a Cena)

### 1. Gancho Inicial (*Hook*) [00:00 - 00:45]

| ⏱️ Tempo | 🖥️ Na Tela (Vídeo / Ação) | 🎙️ Locução (Áudio / O que Falar) |
| :--- | :--- | :--- |
| **00:00** | Tela inicial com um terminal básico escuro comum e sem recursos visuais. Em seguida, transição rápida com corte seco para o **OnyxSH** abrindo com visual Libadwaita moderno, tema escuro e painéis divididos. | *"Se você passa horas do seu dia dentro do terminal Linux gerenciando servidores ou desenvolvendo aplicações, provavelmente já sentiu falta de recursos modernos que outros softwares já têm há anos."* |
| **00:15** | Takes rápidos (cortes dinâmicos de 2 segundos cada):<br>1) Banner vermelho de produção no topo;<br>2) IA analisando saída de erro com 1 clique;<br>3) Interruptor de túnel SSH ativando visualmente. | *"E se o seu terminal tivesse autocomplete inteligente com manual em português, histórico estruturado em SQLite, gerenciador visual de túneis SSH, IA integrada rodando local na sua GPU e até um sistema que impede você de derrubar um servidor de produção por engano?"* |
| **00:30** | Logo do OnyxSH centralizado na tela com texto:<br>**OnyxSH: O Terminal Moderno com IA e SSH para Linux**. | *"Esse é o **OnyxSH**, um emulador de terminal open source construído em GTK4 e Libadwaita projetado especificamente para produtividade máxima no Linux. Vamos conferir cada detalhe dele agora!"* |

---

### 2. Interface Moderna, Abas e Broadcast [00:45 - 01:45]

| ⏱️ Tempo | 🖥️ Na Tela (Vídeo / Ação) | 🎙️ Locução (Áudio / O que Falar) |
| :--- | :--- | :--- |
| **00:45** | Mostra a janela do OnyxSH. Abre novas abas com `Ctrl + Shift + T` e navega rolando o mouse sobre a barra superior. | *"Para começar, o visual: o OnyxSH é nativo, rápido e se integra com perfeição ao ecossistema GNOME e às diretrizes do Libadwaita. Ele traz abas superiores fluidas e organizadas."* |
| **01:05** | Pressiona `Ctrl + Shift + E` (divisão vertical) e `Ctrl + Shift + D` (divisão horizontal). Ajusta o tamanho dos painéis arrastando as bordas. | *"Você pode dividir qualquer aba em múltiplos painéis horizontais ou verticais de forma rápida, ideal para acompanhar logs enquanto compila código ou roda comandos ao lado."* |
| **01:25** | Pressiona `Ctrl + Shift + B`. A barra de **Broadcast** surge no rodapé. Digita `uptime` ou `uname -a`. Mostra o comando sendo enviado simultaneamente para todos os painéis. | *"E quando você precisa executar a mesma instrução em várias máquinas ao mesmo tempo? Pressione `Ctrl + Shift + B`: o modo **Broadcast** espelha o que você digita em todas as abas e painéis abertos instantaneamente."* |

---

### 3. Sessões SSH, Túneis Visuais e Migração do SecureCRT [01:45 - 03:00]

| ⏱️ Tempo | 🖥️ Na Tela (Vídeo / Ação) | 🎙️ Locução (Áudio / O que Falar) |
| :--- | :--- | :--- |
| **01:45** | Abre o painel lateral de sessões. Mostra pastas com ícones organizadas por ambiente (*Produção*, *Staging*, *Infra*). | *"Para quem administra dezenas de servidores, o gerenciador de sessões SSH organiza tudo em pastas hierárquicas. Suporta autenticação por chaves SSH, senhas com armazenamento seguro no chaveiro do sistema e integração nativa com gateways corporativos PAM e Balabit."* |
| **02:15** | Abre o menu: **☰ ➔ Importar Sessões do SecureCRT**. Mostra o diálogo de importação em lote. | *"E se sua equipe ainda usa ferramentas proprietárias legadas como o SecureCRT, você não precisa cadastrar nada do zero: o OnyxSH tem importador automático capaz de ler árvores inteiras de pastas e descriptografar credenciais Password V2 diretamente."* |
| **02:35** | Abre o **Gerenciador de Túneis SSH** (`Ctrl + Shift + P ➔ Túneis`). Mostra lista de túneis Locais (`-L`), Remotos (`-R`) e SOCKS5 Dinâmicos (`-D`). Clica no switch para ativar um túnel em tempo real. | *"Outro recurso indispensável: o **Gerenciador Visual de Túneis SSH**. Chega de ficar decorando parâmetros complexos no terminal. Aqui você configura túneis locais, remotos ou proxies dinâmicos SOCKS5 e ativa ou desativa com um simples clique no interruptor!"* |

---

### 4. Modo Proteção de Produção (Production Guard) [03:00 - 04:00]

| ⏱️ Tempo | 🖥️ Na Tela (Vídeo / Ação) | 🎙️ Locução (Áudio / O que Falar) |
| :--- | :--- | :--- |
| **03:00** | Abre uma conexão com flag de produção ativada. Dá zoom na barra superior com gradiente crimson de alta visibilidade: `🛡️ PRODUÇÃO`. | *"Agora, um dos recursos mais inovadores: o **Production Guard**. Quantas vezes alguém já gelou a espinha com medo de rodar um comando na aba errada? Ao conectar em um servidor crítico, essa barra de alerta vermelha persistente deixa o perigo evidente."* |
| **03:20** | Digita no terminal: `rm -rf /var/log/app` ou `systemctl stop nginx` e tecla `Enter`. Um diálogo de confirmação crítica salta na tela com bloqueio de execução. | *"E não para por aí: se você tentar executar comandos destrutivos como `rm -rf`, `DROP DATABASE`, `dd` ou tentar desligar a máquina, o OnyxSH intercepta a ação antes de chegar ao shell! Ele exige que você digite o nome exato do servidor para autorizar. É a proteção definitiva contra acidentes operacionais."* |

---

### 5. Autocomplete com Specs Linux e Histórico SQLite [04:00 - 05:00]

| ⏱️ Tempo | 🖥️ Na Tela (Vídeo / Ação) | 🎙️ Locução (Áudio / O que Falar) |
| :--- | :--- | :--- |
| **04:00** | Começa a digitar `docker run ` ou `systemctl `. O popup de sugestões flutuante surge ancorado ao cursor com as opções, flags explicadas e ícones. | *"A produtividade na linha de comando é elevada com o **Autocomplete Inteligente**. Ele traz um catálogo nativo com mais de 50 comandos Linux essenciais. Conforme você digita, uma janela flutuante ancorada ao cursor sugere parâmetros, flags e descrições claras em português."* |
| **04:25** | Pressiona `Ctrl + H`. Abre a tela de **Histórico Enriquecido**. Mostra os filtros por pílulas (*Diretório Atual*, *Host Remoto*, *Favoritos ⭐*). | *"Esqueça o `history` puro do bash. Com `Ctrl + H`, você acessa o histórico enriquecido gravado em banco SQLite. Você pode filtrar apenas comandos executados no diretório atual, na máquina remota ativa ou marcar comandos complexos com estrela ⭐ para acesso rápido."* |
| **04:45** | Pressiona `Ctrl + Shift + P` e digita termos como *"Tema"*, *"Novo Terminal"*, *"Exportar"* ou o nome de um host. | *"E no estilo VS Code, a **Command Palette** (`Ctrl + Shift + P`) permite navegar e executar qualquer ação do aplicativo sem tirar as mãos do teclado."* |

---

### 6. IA Integrada, Diagnóstico de Erros e Modo Agente [05:00 - 06:15]

| ⏱️ Tempo | 🖥️ Na Tela (Vídeo / Ação) | 🎙️ Locução (Áudio / O que Falar) |
| :--- | :--- | :--- |
| **05:00** | Roda um comando que retorna erro (ex.: erro de compilação ou sintaxe). Um badge surge ao lado do prompt. Clica em **Analisar com IA** e o chat lateral abre com a explicação pronta. | *"O OnyxSH traz suporte de ponta a Inteligência Artificial. Se um comando falhar com erro de sintaxe ou permissão, um badge surge no prompt: com um clique a IA analisa a mensagem de saída e já explica exatamente como corrigir."* |
| **05:30** | Abre a tela de preferências da IA (`Ctrl + Shift + I ➔ ⚙️`). Mostra os provedores: **Ollama local**, **Gemini**, **Groq** e **OpenRouter**, além do monitor de VRAM da GPU. | *"O melhor: privacidade total! Você pode rodar modelos 100% locais via **Ollama**. O OnyxSH detecta a VRAM da sua placa NVIDIA ou AMD, ajusta o contexto ideal, faz pré-carregamento para resposta instantânea e libera a memória de vídeo ao fechar o app."* |
| **05:55** | Demonstra uma automação via IA. A tela exibe a política de segurança, o visualizador de *Diff* lado a lado e o botão de aprovação. | *"E no Modo Agente para automações, nada é executado às cegas: você visualiza um diff das alterações, conta com aprovação explícita e rollback automático caso queira reverter qualquer arquivo."* |

---

### 7. SFTP, Edição Remota e Caderno de Bordo (Runbook) [06:15 - 07:15]

| ⏱️ Tempo | 🖥️ Na Tela (Vídeo / Ação) | 🎙️ Locução (Áudio / O que Falar) |
| :--- | :--- | :--- |
| **06:15** | Abre o painel lateral de arquivos **SFTP** em uma aba SSH. Arrasta um arquivo local para dentro do painel. Em seguida, clica com botão direito em um arquivo remoto ➔ **Editar Arquivo**. O arquivo abre no editor local (VS Code/Gedit), é salvo e sincronizado de volta. | *"Você também não precisa de clientes externos como FileZilla. O painel lateral SFTP aceita arrastar e soltar e permite **edição remota transparente**: você abre o arquivo no seu editor local favorito, salva, e o OnyxSH sobe a alteração automaticamente para o servidor."* |
| **06:45** | Abre o menu **Exportar Terminal / Caderno de Bordo**. Mostra a prévia do relatório em HTML Dark estilizado e Markdown formatado com detalhes expansíveis. | *"E para relatórios técnicos, post-mortems e documentação de procedimentos: o **Caderno de Bordo (Runbook)** transforma suas sessões de terminal em relatórios executivos em HTML ou Markdown prontos para compartilhar com sua equipe."* |

---

### 8. Conclusão, Como Instalar e Chamada para Ação [07:15 - 08:00]

| ⏱️ Tempo | 🖥️ Na Tela (Vídeo / Ação) | 🎙️ Locução (Áudio / O que Falar) |
| :--- | :--- | :--- |
| **07:15** | Exibe a página do repositório no GitHub (`VaGNaroK/OnyxSH`) e destaca os comandos de instalação via Flatpak e `.deb`. | *"O OnyxSH é totalmente gratuito e open source sob licença GPLv3. Está disponível para qualquer distribuição Linux via **Flatpak** pelo Flathub, em pacote **.deb** para Debian, Ubuntu e Linux Mint, além do instalador universal."* |
| **07:35** | Tela final com links úteis, redes sociais e botões animados de Like e Inscreva-se. | *"Todos os links oficiais estão aqui na descrição do vídeo. Deixe sua estrela no repositório do GitHub para apoiar o projeto, comente qual recurso você mais gostou e não esqueça do like e de se inscrever no canal. Até a próxima!"* |

---

## 📌 Guia Rápido de Atalhos para Gravação

Tenha esta colinha aberta durante a gravação para demonstrar a velocidade do app:

| Atalho | Ação Demonstrada no Vídeo |
| :--- | :--- |
| `Ctrl + Shift + T` | Abrir nova aba |
| `Ctrl + Shift + E` | Dividir tela verticalmente |
| `Ctrl + Shift + D` | Dividir tela horizontalmente |
| `Ctrl + Shift + B` | Alternar modo Broadcast (digitação simultânea) |
| `Ctrl + H` | Abrir Histórico Enriquecido SQLite |
| `Ctrl + Shift + P` | Abrir Command Palette (busca spotlight) |
| `Ctrl + Shift + I` | Abrir painel do Assistente de IA |
| `Alt + Up` / `Alt + Down` | Saltar entre prompts de comandos anteriores/posteriores |
| `Ctrl + Shift + F` | Busca no scrollback com Regex |

---

## 📋 Textos Prontos para Postagem

### Sugestões de Título
1. **OnyxSH: O Terminal Linux Definitivo com IA, Túneis SSH e Proteção de Produção!** *(Mais recomendado)*
2. **Adeus Terminais Legados! Conheça o OnyxSH (Terminal GTK4 com IA e SSH)**
3. **Esse Novo Terminal Linux Impede Você de Destruir Servidores de Produção por Engano!**

### Descrição do Vídeo (YouTube / LinkedIn / Blog)

```markdown
Apresentando o OnyxSH: um emulador de terminal moderno construído em GTK4 e Libadwaita para desenvolvedores, DevOps e SysAdmins.

Neste vídeo você confere um tour completo pelas principais funcionalidades:
- 🛡️ Production Guard: Banner de alerta crimson e interceptação de comandos perigosos (rm -rf, DROP DATABASE, reboot)
- 🌐 Gerenciador Visual de Túneis SSH: Redirecionamento Local (-L), Remoto (-R) e SOCKS5 Dinâmico (-D) com switch de 1 clique
- 🤖 Assistente de IA Integrado: Diagnóstico de erros com modelos locais na GPU (Ollama) ou nuvem (Gemini, Groq), com gestão de VRAM
- ⚡ Autocomplete Inteligente: Sugestões ancoradas ao cursor com manual explicativo de mais de 50 comandos Linux
- 🔍 Histórico Enriquecido SQLite (Ctrl + H): Filtros por diretório, host remoto e comandos favoritos ⭐
- 📁 Painel SFTP Integrado: Drag & drop e edição remota transparente no seu editor local favorito
- 📘 Caderno de Bordo (Runbook): Exportação de procedimentos e relatórios em HTML, Markdown e Asciinema
- ⌨️ Command Palette (Ctrl + Shift + P) e Modo Broadcast (Ctrl + Shift + B)

🔗 Repositório Oficial no GitHub:
https://github.com/VaGNaroK/OnyxSH

📦 Formas de Instalação:
• Flatpak (Universal):
  flatpak run io.github.vagnarok.OnyxSH
• Debian / Ubuntu / Linux Mint (.deb):
  sudo apt install ./dist/onyxsh_0.13.0_all.deb
• Instalador Universal:
  ./install.sh install

Gostou do projeto? Deixe sua estrela ⭐ no GitHub para apoiar o software livre!

#Linux #Terminal #DevOps #SysAdmin #OpenSource #OnyxSH #SSH #GTK4 #Libadwaita #Python
```
