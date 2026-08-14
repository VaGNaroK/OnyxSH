<p align="right">
  <strong>🇧🇷 Português</strong> | <a href="README.en.md">🇺🇸 English</a>
</p>

# Zashterminal

<p align="center">
  <img src="https://github.com/VaGNaroK/zashterminal-Fork/blob/main/usr/share/icons/hicolor/scalable/apps/zashterminal.svg" alt="Logo Zashterminal" width="128" height="128">
</p>

<p align="center">
  <strong>Um emulador de terminal moderno para desenvolvedores, infraestrutura e administração de sistemas</strong>
</p>
<p align="center">
  <a href="https://github.com/VaGNaroK/zashterminal-Fork/blob/main/LICENSE"><img src="https://img.shields.io/badge/Licen%C3%A7a-GPL--3.0-green.svg" alt="Licença"/></a>
  <a href="https://www.gtk.org/"><img src="https://img.shields.io/badge/GTK-4.0+-orange.svg" alt="Versão GTK"/></a>
  <a href="https://gnome.pages.gitlab.gnome.org/libadwaita/"><img src="https://img.shields.io/badge/libadwaita-1.0+-purple.svg" alt="Versão libadwaita"/></a>
</p>

> [!NOTE]
> Este projeto é um fork aprimorado do [Zashterminal](https://github.com/leoberbert/zashterminal) com foco em segurança avançada, **Modo Agente Seguro (Zero Direct Execution)**, integração Polkit, trilha de auditoria append-only, rollback byte-identical e instalador unificado.

O **Zashterminal** é um terminal moderno, intuitivo e poderoso construído em GTK4 e Libadwaita. Ele combina recursos avançados para desenvolvedores e administradores de sistemas (DevOps/SRE) com uma interface amigável e acessível. Gerenciamento de sessões integrado, painel lateral de arquivos remotos, realce de sintaxe em tempo real e ferramentas focadas em produtividade tornam o uso do shell muito mais eficiente no Linux.

---

## Por que o Zashterminal?

- **Focado em Fluxos de Trabalho Reais**: Gerencie sessões SSH/SFTP, divisões de painéis (*split panes*) e layouts sem sair da janela do terminal.
- **Acessível e Intuitivo**: Interface limpa, configurações inteligentes e atalhos fáceis de descobrir.
- **Assistência de IA Segura e Opcional**: Somente o texto que você selecionar explicitamente é enviado, mantendo privacidade e controle total.
- **Modo Agente Seguro**: Execução de planos assistidos por IA mediada por políticas, confirmações visuais de diff, elevação Polkit e rollback automático.
- **Visual Moderno e Nativo**: Construído em GTK4 + Libadwaita com suporte a temas claros e escuros e transparência fluida.

---

## Migração do SecureCRT & Compatibilidade PAM

Facilite a migração de ferramentas legadas para o Zashterminal:

- **Importação Direta de Sessões do SecureCRT**: Importe sessões pelo menu principal (`Importar Sessões do SecureCRT`).
- **Importação em Lote de Árvores de Diretórios**: Suporte a pastas completas com arquivos `.ini`.
- **Compatibilidade com Password V2 do SecureCRT**: Reconhece e descriptografa entradas `02:<hex>` mantendo compatibilidade de credenciais.
- **Suporte a Gateways Balabit / One Identity**: Compatível com fluxos de autenticação *keyboard-interactive* amplamente utilizados em ambientes corporativos de Privileged Access Management (PAM).

---

## Capturas de Tela

<img width="1457" height="699" alt="Interface Principal do Zashterminal" src="https://github.com/user-attachments/assets/4c264548-909e-4edb-95be-a5dc6a6756bb" />

<img width="1457" height="699" alt="Gerenciador de Sessões e Painéis" src="https://github.com/user-attachments/assets/6aba3c63-a181-4e3c-8870-d58ceae11daa" />

<img width="1457" height="699" alt="Painel Lateral de Arquivos" src="https://github.com/user-attachments/assets/46e41739-7c28-47d7-b4ba-26e9320b0061" />

---

## Principais Recursos

### 🤖 Assistente de IA Integrado

<img width="1457" height="699" alt="Painel do Assistente de IA" src="https://github.com/user-attachments/assets/762fa599-a266-41c3-83c2-f28fe825f0f6" />

<img width="1457" height="699" alt="Sugestões e Execução de Comandos" src="https://github.com/user-attachments/assets/4dd9482b-420d-4170-878d-e9a652493ec9" />

O Zashterminal integra Modelos de Linguagem (LLMs) ao terminal de forma não-intrusiva e com foco estrito em privacidade:
* **Múltiplos Provedores**: Suporte nativo a **Groq**, **Google Gemini**, **OpenRouter** e **Modelos Locais** (Ollama / LM Studio).
* **Consciência de Contexto**: Compreende a distribuição Linux em uso para sugerir comandos específicos e corretos.
* **Painel Lateral Dedicado**: Histórico de conversas, sugestões de comandos e botões para executar com um clique.

---

### 🛡️ Modo Agente Seguro (Secure Agent Mode)

O **Modo Agente Seguro** permite ao assistente de IA planejar e realizar tarefas complexas com supervisão do usuário e garantias de segurança rigorosas.

Ao contrário de agentes tradicionais que executam comandos arbitrários no shell, o Zashterminal adota uma arquitetura de **Zero Direct Execution**:

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

#### Níveis de Risco Estratificados

| Nível | Categoria | Exemplos | Mecanismo de Aprovação |
|---|---|---|---|
| 🟢 **Nível 0** | Leitura Segura | `ls`, `df -h`, `free -m`, `uptime`, `ip route` | **1 Clique:** `[▶ Executar]` ou `[🧪 Simular]` |
| 🔵 **Nível 1** | Escrita no Usuário | Criação e edição de arquivos na home | **Revisão de Diff:** visualização unificada com backup automático prévio |
| 🟠 **Nível 2** | Administração do Sistema | Limpeza de logs do journal, manutenção de pacotes | **Elevação Polkit:** autenticação gráfica via `zashterminal-admin-helper` |
| 🔴 **Nível 3** | Ação Crítica | Desinstalação de dependências do sistema | **Confirmação Explícita** |
| ⛔ **Nível 4** | Bloqueado / Proibido | `rm -rf /`, `mkfs.*`, `dd of=/dev/sd*`, `chmod 777 /` | **Bloqueio Intransponível:** botão desabilitado na interface |

#### Garantias de Segurança
- **Isolamento de Contexto:** Saídas externas e terminais são envelopadas em `<untrusted>...</untrusted>` para prevenir injeções indiretas de prompt.
- **Redator Automático de Segredos:** Mascara chaves de API, chaves privadas RSA/PGP e credenciais antes do envio para modelos remotos.
- **PathGuard Anti-Bypass:** Bloqueia leitura e escrita em credenciais (`~/.ssh`, `~/.aws`, `.env`) e dotfiles de inicialização (`.bashrc`, `.zshrc`), resolvendo links simbólicos antes da checagem.
- **Trilha de Auditoria e Rollback:** Histórico contínuo em `audit.jsonl` com possibilidade de reverter alterações de arquivos com integridade SHA-256 garantida.

Consulte o documento completo em [docs/SECURITY.md](docs/SECURITY.md) para detalhes técnicos.

---

### 📂 Gerenciador de Arquivos e Edição Remota

<img width="1457" height="699" alt="Navegação de Arquivos" src="https://github.com/user-attachments/assets/a40bd623-eb31-4a8b-9fe2-e327d8b7de0c" />

- **Painel de Arquivos Integrado**: Navegue no sistema de arquivos local e remoto sem precisar de ferramentas externas.
- **Edição Remota Transparente**: Abra arquivos remotos no seu editor local; ao salvar, as alterações são sincronizadas automaticamente via SFTP/SCP.
- **Transferência por Arraste (Drag & Drop)**: Envie arquivos para servidores remotos arrastando-os para o terminal.
- **Gerenciador de Transferências**: Acompanhe o progresso de uploads e downloads.

---

### ⚡ Produtividade e Administração

<img width="1457" height="699" alt="Broadcast de Comandos" src="https://github.com/user-attachments/assets/97aae8ed-6466-46b9-b7e4-ca1256f425ff" />

- **Broadcast de Entrada**: Digite comandos em um terminal e replique-os simultaneamente em várias abas/painéis selecionados.
- **Quick Prompts**: Ações de um clique para diagnóstico rápido (ex: "Explicar este erro", "Otimizar comando").
- **Gerenciamento de Sessões**: Salve e organize conexões Locais, SSH e SFTP em pastas personalizadas.
- **Divisão de Telas e Layouts**: Divida painéis horizontal e verticalmente e salve layouts complexos.
- **Rastreamento de Diretório (OSC7)**: Atualiza o título das abas automaticamente conforme o diretório de trabalho.
- **Realce de Sintaxe em Tempo Real**: Mais de 50 regras integradas (docker, git, systemctl, kubectl, etc.).

---

## 📥 Instalação

### Arch Linux / Manjaro

```bash
# Via AUR:
yay -S zashterminal        # ou paru -S zashterminal
```

### Debian / Ubuntu / Linux Mint / Fedora / openSUSE

O script instalador detecta a distribuição, instala os pacotes do sistema necessários e configura o Zashterminal em `/opt/zashterminal/venv`:

```bash
# Instalação rápida:
curl -fsSL https://raw.githubusercontent.com/VaGNaroK/zashterminal-Fork/refs/heads/main/install.sh | bash

# Ou baixe e execute:
curl -fsSLO https://raw.githubusercontent.com/VaGNaroK/zashterminal-Fork/refs/heads/main/install.sh
chmod +x install.sh
./install.sh install
```

### NixOS

No NixOS, o instalador utiliza a flake do projeto (`flake.nix` / `default.nix`) e instala o pacote no perfil do usuário:

```bash
curl -fsSL https://raw.githubusercontent.com/VaGNaroK/zashterminal-Fork/refs/heads/main/install.sh | bash
```

### WSL no Windows (Experimental)

```bash
curl -fsSL https://raw.githubusercontent.com/VaGNaroK/zashterminal-Fork/refs/heads/main/install.sh | bash
```

---

## 💻 Uso

```bash
zashterminal [opções] [diretório]
```

### Opções da Linha de Comando

| Opção | Descrição |
|---|---|
| `-w, --working-directory DIR` | Define o diretório inicial de trabalho |
| `-e, -x, --execute COMANDO` | Executa um comando ao iniciar |
| `--close-after-execute` | Fecha a aba após a execução do comando terminar |
| `--ssh [USUARIO@]HOST` | Conecta-se diretamente a um host SSH |
| `--new-window` | Força a abertura em uma nova janela em vez de aba |

### Exemplos

```bash
# Abrir no diretório do projeto
zashterminal ~/projetos

# Conectar diretamente a um servidor SSH
zashterminal --ssh usuario@servidor.exemplo.com

# Executar comando e fechar após término
zashterminal --close-after-execute -e "htop"
```

---

## ⚙️ Arquivos de Configuração

As configurações são salvas em `~/.config/zashterminal/`:

| Arquivo / Pasta | Descrição |
|---|---|
| `settings.json` | Preferências gerais, aparência, atalhos e configurações de IA |
| `sessions.json` | Conexões salvas de SSH/SFTP e pastas de sessão |
| `session_state.json` | Estado das abas e restauração de sessão |
| `layouts/` | Layouts de janelas e divisão de painéis salvos |
| `backups/` | Backups e arquivos de manifesto |

---

## 🤝 Contribuindo

Contribuições, correções e sugestões são muito bem-vindas!

1. Faça um Fork do projeto.
2. Crie uma branch para sua funcionalidade (`git checkout -b feature/minha-melhoria`).
3. Commit suas alterações (`git commit -m 'feat: adiciona nova funcionalidade'`).
4. Envie para o branch (`git push origin feature/minha-melhoria`).
5. Abra um Pull Request.

---

## 📄 Licença

Este projeto é distribuído sob a licença **GNU General Public License v3 (GPLv3)** — consulte o arquivo [LICENSE](LICENSE) para detalhes completos.

---

## 👏 Créditos e Agradecimentos

- **Projeto Original:** Este repositório é um fork do projeto [Zashterminal](https://github.com/leoberbert/zashterminal), criado originalmente por **Leonardo Berbert**.
- Agradecimentos aos desenvolvedores e comunidades do **GNOME**, **GTK**, **libadwaita**, **VTE** e **Pygments**.
