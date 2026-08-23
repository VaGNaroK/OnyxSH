<p align="right">
  <strong>🇧🇷 Português</strong> | <a href="README.en.md">🇺🇸 English</a>
</p>

# OnyxSH

<p align="center">
  <img src="usr/share/icons/hicolor/scalable/apps/io.github.vagnarok.OnyxSH.svg" alt="Logo OnyxSH" width="128" height="128">
</p>

<p align="center">
  <strong>Um emulador de terminal moderno com IA, SSH e inteligência semântica para desenvolvedores e administradores de sistemas</strong>
</p>
<p align="center">
  <a href="https://github.com/VaGNaroK/OnyxSH/blob/main/LICENSE"><img src="https://img.shields.io/badge/Licen%C3%A7a-GPL--3.0-green.svg" alt="Licença"/></a>
  <a href="https://www.gtk.org/"><img src="https://img.shields.io/badge/GTK-4.0+-orange.svg" alt="Versão GTK"/></a>
  <a href="https://gnome.pages.gitlab.gnome.org/libadwaita/"><img src="https://img.shields.io/badge/libadwaita-1.0+-purple.svg" alt="Versão libadwaita"/></a>
</p>

> [!NOTE]
> O **OnyxSH** é uma evolução independente e hard fork do [Zashterminal](https://github.com/leoberbert/zashterminal) (criado originalmente por Leonardo Berbert). O projeto traz uma identidade renovada com integração profunda de Inteligência Artificial, rastreamento semântico de shell (OSC 133), histórico enriquecido de comandos em SQLite, Command Palette, restauração automática de sessões e suporte multilíngue em 28 idiomas.

O **OnyxSH** é um terminal moderno, intuitivo e de alto desempenho construído em GTK4 e Libadwaita. Ele combina recursos avançados para desenvolvedores e administradores de sistemas (DevOps/SRE) com uma interface nativa elegante, rápida e limpa.

---

## Por que o OnyxSH?

- **Assistência de IA Integrada e Privativa**: Diagnóstico de erros com 1 clique, múltiplos provedores (Ollama local, Gemini, Groq, OpenRouter), pré-carregamento e gestão inteligente de VRAM.
- **Gerenciador Visual de Túneis SSH & Port Forwarding**: Gerenciamento em tempo real de túneis Locais (`-L`), Remotos (`-R`) e SOCKS5 Dinâmicos (`-D`) com switches de ativação em 1 clique.
- **Modo Proteção de Produção (Production Guard)**: Banner visual persistente, salvaguardas de privacidade e interceptação automática de comandos destrutivos em servidores de produção.
- **Autocomplete Inteligente com Specs Linux**: Sugestões ricas ancoradas ao cursor com descrições declarativas de mais de 50 comandos Linux, histórico e templates de snippets.
- **Integração Semântica com Shell (OSC 133 / OSC 6)**: Rastreia o ciclo de vida de cada comando, mede o tempo de execução (ex: `⏱ 1.4s`), permite saltos rápidos entre prompts (`Alt + Up` / `Alt + Down`) e extração cirúrgica de saída.
- **Histórico Enriquecido em SQLite (`Ctrl + H`)**: Busca fuzzy instantânea com filtros contextuais (diretório atual, host remoto, favoritos ⭐), contadores de execução, opções de limpeza e inserção no prompt (`Tab`).
- **Command Palette Spotlight (`Ctrl + Shift + P`)**: Busca rápida e execução de qualquer comando, ação de aba, túnel SSH, configuração ou sessão via teclado.
- **Exportação Multi-formato do Terminal**: Exporte buffers ou seleções do terminal em `.txt`, `.log`, `.md`, `.html` e `.cast` (Asciinema).
- **Restauração Automática de Sessões**: Restaura abas, painéis divididos (*splits*), diretórios correntes `$PWD` e sessões SSH entre inicializações.
- **Gestão Completa de Conexões Remotas**: Árvore de sessões SSH/SFTP em pastas, servidor TFTP integrado, transferência Drag & Drop e edição remota transparente de arquivos.
- **Visual Moderno e Nativo**: Construído com Libadwaita, suporte a modo escuro/claro, transparência e temas de cores customizáveis.
- **Internacionalização Completa**: Traduzido e sincronizado em **28 idiomas**.

📖 **Confira o [Manual Completo de Uso](docs/MANUAL.md)** para um guia detalhado de todas as funcionalidades!

---

## Migração do SecureCRT & Compatibilidade PAM

- **Importação Direta de Sessões do SecureCRT**: Importe sessões pelo menu principal (`Importar Sessões do SecureCRT`).
- **Importação em Lote de Árvores de Diretórios**: Suporte a pastas completas com arquivos `.ini`.
- **Compatibilidade com Password V2 do SecureCRT**: Reconhece e descriptografa entradas `02:<hex>` mantendo compatibilidade de credenciais.
- **Suporte a Gateways Balabit / One Identity**: Compatível com fluxos de autenticação *keyboard-interactive* amplamente utilizados em ambientes corporativos de Privileged Access Management (PAM).

---

## Capturas de Tela

<img width="1457" height="699" alt="Interface Principal do OnyxSH" src="https://github.com/user-attachments/assets/4c264548-909e-4edb-95be-a5dc6a6756bb" />

<img width="1457" height="699" alt="Gerenciador de Sessões e Painéis" src="https://github.com/user-attachments/assets/6aba3c63-a181-4e3c-8870-d58ceae11daa" />

<img width="1457" height="699" alt="Painel Lateral de Arquivos" src="https://github.com/user-attachments/assets/46e41739-7c28-47d7-b4ba-26e9320b0061" />

---

## Principais Recursos

### 🌐 Gerenciador Visual de Túneis SSH e Port Forwarding
* **3 Modos de Redirecionamento**:
  - 🟢 **Local Forwarding (`-L`)**: Acesso a bancos de dados, dashboards e serviços internos remotos via portas locais.
  - 🔄 **Remote Forwarding (`-R`)**: Exposição de portas locais diretamente em servidores remotos.
  - 🛡️ **Dynamic Port Forwarding (`-D`)**: Criação instantânea de proxy SOCKS5 local criptografado via SSH.
* **Painel de Controle em Tempo Real**: Ativação e desativação em 1 clique com `Gtk.Switch`, cópia de endereço local e botão de parada global.
* **Auto-start com a Sessão**: Inicialização automática de túneis vinculados ao abrir a sessão SSH correspondente.

---

### 🛡️ Modo Proteção de Produção (Production Guard)
* **Banner de Produção**: Faixa visual persistente de alta visibilidade em degradê crimson no topo de abas conectadas a servidores de produção.
* **Bloqueio de Comandos Destrutivos**: Intercepta comandos críticos como `rm -rf`, `mkfs`, `dd of=/dev/...`, `shutdown`, `reboot`, `systemctl stop`, `DROP DATABASE` e `git push --force`.
* **Confirmação Dupla Segura**: Exige a digitação do nome exato do host/sessão antes de desbloquear a execução.
* **Ofuscação de Segredos**: Mascaramento automático de chaves de API, senhas e credenciais antes do envio para modelos de IA.

---

### ⚡ Autocomplete Inteligente com Catálogo de Specs Linux
* **Popup Flutuante Ancorado ao Cursor**: Dicionário nativo com mais de 50 comandos Linux essenciais (`apt`, `docker`, `git`, `systemctl`, `curl`, `rsync`, `chmod`, etc.), com descrições e flags explicadas.
* **Sugestões Contextuais**: Ranquemento inteligente combinando specs oficiais, comandos frequentes do histórico SQLite no mesmo diretório e snippets salvos.
* **Navegação Eficiente**: Navegue com `↑` e `↓` e complete com `Tab` ou `Enter`.

---

### 🤖 Assistente de IA Integrado & Gestão de VRAM

<img width="1457" height="699" alt="Painel do Assistente de IA" src="https://github.com/user-attachments/assets/762fa599-a266-41c3-83c2-f28fe825f0f6" />

* **Múltiplos Provedores**: Suporte nativo a **Modelos Locais** (Ollama / LM Studio), **Groq**, **Google Gemini** e **OpenRouter**.
* **Detecção Automática de GPU & VRAM**: Reconhece placas NVIDIA (`nvidia-smi`), AMD/Intel (DRM sysfs) e memória do sistema, calculando o limite seguro de contexto.
* **Seletor de Janela de Contexto (4K a 128K tokens)**: Permite ajustar a janela de contexto enviada ao Ollama (`num_ctx`) com recomendação dinâmica baseada na VRAM da GPU.
* **Ciclo de Vida Inteligente de VRAM**:
  - **Pré-carregamento Assíncrono:** Carrega o modelo local na VRAM em segundo plano ao abrir o terminal (zero espera no primeiro comando).
  - **Descarregamento Automático:** Libera a VRAM da GPU imediatamente ao fechar o terminal.
* **Diagnóstico de Erros em 1 Clique**: Ao ocorrer um erro no shell (`exit_code != 0`), um badge interativo permite enviar a saída diretamente para análise com a IA.

---

### 🔍 Command Palette (`Ctrl + Shift + P`) & Histórico SQLite (`Ctrl + H`)

* **Command Palette**: Diálogo modal spotlight que indexa todas as ações da interface, abas, layouts, assistente de IA, regras de realce, túneis e sessões SSH com pesquisa fuzzy.
* **Histórico Enriquecido**: Gravação estruturada em SQLite de cada comando executado, diretório `$PWD`, duração, data/hora e status.
  - **Filtros por Pílulas:** *Todos*, *Diretório Atual*, *Host Remoto*, *⭐ Favoritos*.
  - **Atalhos Rápidos:** `Enter` (executar), `Tab` (inserir no prompt para edição), `Ctrl + P` (fixar como favorito), `Delete` (excluir item), `Ctrl + Shift + Delete` (abrir opções de limpeza).
  - **Limpeza Flexível:** Limpe comandos não favoritados, comandos com falha ou todo o histórico com 1 clique.

---

### 📤 Exportação do Terminal em Múltiplos Formatos
* Exporte o histórico do terminal ou apenas o texto selecionado em 5 formatos:
  - 📄 **Texto Puro (`.txt`)**
  - 📋 **Arquivo de Log (`.log`)** com metadados completos de sessão
  - 📝 **Markdown (`.md`)** formatado em blocos de código
  - 🌐 **HTML (`.html`)** com preservação de cores ANSI e estilo dark moderno
  - 🎬 **Asciinema (`.cast`)** para reprodução e compartilhamento de gravações de terminal

---

### 🛡️ Modo Agente Seguro (Zero Direct Execution)

Arquitetura de segurança estrita para execução assistida por IA:

```
[ Usuário ] ── Solicitação ──▶ [ LLM Provider (Groq / Gemini / Ollama) ]
                                            │
                                  Gera ActionPlan JSON
                                            ▼
                                   [ PolicyEngine ]
                              (Classificação 0-4 + Denylists)
                                            │
                                            ▼
                               [ UI: Aprovação do Usuário ]
                                            │
                        ┌───────────────────┴───────────────────┐
                        ▼                                       ▼
            🟢 Nível 0 (1-clique)                   🔵 Nível 1 (Diff + Backup)
            🟠 Nível 2 (Polkit Admin)               ⛔ Nível 4 (Bloqueado)
                        │                                       │
                        └───────────────────┬───────────────────┘
                                            ▼
                                    [ ToolRegistry ]
                                            │
                                            ▼
                             [ AuditLog + Rollback JSONL ]
```

- **Redator Automático de Segredos:** Mascara chaves de API, chaves privadas SSH e credenciais antes do envio para modelos remotos.
- **PathGuard Anti-Bypass:** Bloqueia leitura e escrita em credenciais (`~/.ssh`, `~/.aws`, `.env`) e dotfiles de inicialização (`.bashrc`, `.zshrc`).
- **Trilha de Auditoria e Rollback:** Histórico contínuo em `audit.jsonl` com reversão de alterações com integridade SHA-256 garantida.

---

## 📥 Instalação & Empacotamento

### 📦 Flatpak (Recomendado para Qualquer Distribuição Linux)

```bash
# Instalar o bundle gerado:
flatpak install --user -y --reinstall dist/onyxsh_0.9.0.flatpak

# Executar:
flatpak run io.github.vagnarok.OnyxSH
```

### 📦 Pacote Debian (.deb - Ubuntu, Linux Mint, Debian)

```bash
sudo apt install ./dist/onyxsh_0.9.0_all.deb
```

### ⚡ Instalador Universal & Empacotamento Híbrido (`install.sh`)

```bash
# Instalar no sistema:
./install.sh install

# Gerar pacote Flatpak:
./scripts/build_flatpak.sh --clean-cache

# Gerar pacote .deb:
./scripts/build_deb.sh --clean-cache
```

---

## 💻 Atalhos Padrão

| Atalho | Ação |
|---|---|
| **`F2`** | Abrir Janela de Preferências |
| **`Ctrl + H`** | Histórico Enriquecido de Comandos (SQLite) |
| **`Ctrl + Shift + P`** | Command Palette (Spotlight) |
| **`Alt + Up` / `Alt + Down`** | Navegar entre Prompts Anteriores e Posteriores |
| **`Ctrl + Shift + T`** | Nova Aba |
| **`Ctrl + Shift + W`** | Fechar Aba / Painel Ativo |
| **`Ctrl + Shift + D`** | Dividir Terminal Horizontalmente |
| **`Ctrl + Shift + E`** | Dividir Terminal Verticalmente |
| **`Ctrl + Shift + F`** | Buscar no Scrollback do Terminal (com suporte a Regex) |
| **`Ctrl + Shift + I`** | Abrir Painel do Assistente de IA |
| **`Ctrl + Shift + B`** | Transmissão de Comandos para Todas as Abas (*Broadcast*) |

---

## ⚙️ Arquivos de Configuração

As configurações são salvas em `~/.config/onyxsh/` (com migração automática transparente de `~/.config/zashterminal`):

| Arquivo / Pasta | Descrição |
|---|---|
| `settings.json` | Preferências gerais, aparência, atalhos e configurações de IA |
| `sessions.json` | Conexões salvas de SSH/SFTP e pastas de sessão |
| `command_history.db` | Banco de dados SQLite do histórico enriquecido de comandos |
| `session_state.json` | Estado das abas e restauração de sessão |
| `layouts/` | Layouts de janelas e divisão de painéis salvos |
| `backups/` | Backups e arquivos de manifesto |

---

## 🤝 Contribuindo

Contribuições, correções e sugestões são muito bem-vindas!

1. Faça um Fork do projeto (`https://github.com/VaGNaroK/OnyxSH`).
2. Crie uma branch para sua funcionalidade (`git checkout -b feature/minha-melhoria`).
3. Commit suas alterações (`git commit -m 'feat: adiciona nova funcionalidade'`).
4. Envie para o branch (`git push origin feature/minha-melhoria`).
5. Abra um Pull Request.

---

## 📄 Licença

Este projeto é distribuído sob a licença **GNU General Public License v3 (GPLv3)** — consulte o arquivo [LICENSE](LICENSE) para detalhes completos.

---

## 👏 Créditos e Agradecimentos

- **Projeto Original:** Baseado originalmente no [Zashterminal](https://github.com/leoberbert/zashterminal), criado por **Leonardo Berbert**.
- Agradecimentos aos desenvolvedores e comunidades do **GNOME**, **GTK**, **libadwaita**, **VTE** e **Pygments**.
