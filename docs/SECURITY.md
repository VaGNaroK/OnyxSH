# Modelo de Segurança e Ameaças — Zashterminal Modo Agente Seguro

Este documento descreve o modelo de segurança, ameaças consideradas e as respectivas mitigações implementadas no **Modo Agente Seguro do Zashterminal**.

---

## 1. Princípios Fundamentais de Arquitetura

1. **Separação Estrita de Planos e Execução:**
   - Modelos de Linguagem (LLMs) **nunca** executam comandos diretamente no sistema operacional.
   - O LLM atua exclusivamente como planejador, gerando planos estruturados (`ActionPlan`) em formato JSON.
   - Toda execução é mediada pelo **Motor de Políticas (`PolicyEngine`)**, aprovação do usuário e **Registro de Ferramentas (`ToolRegistry`)**.

2. **Subprocessos sem Interpolação Shell (`shell=False`):**
   - 100% das invocações de processos utilizam `subprocess` com `shell=False` e vetores de argumentos explícitos (`list[str]`).
   - Invocações do tipo `sh -c` ou `bash -c` são estritamente proibidas e bloqueadas.

3. **Postura Default-Deny:**
   - Diretórios sensíveis, credenciais e arquivos de inicialização do sistema são protegidos por padrão.
   - Ações administrativas não autorizadas explicitamente em `admin_actions.json` são terminantemente rejeitadas.

4. **Tratamento de Dados Externos como Não-Confiáveis:**
   - Saídas de terminal, conteúdos de arquivos inspecionados e históricos são envelopados em tags `<untrusted source="...">...</untrusted>` no prompt de contexto.

---

## 2. Matriz de Ameaças e Mitigações

| Vetor de Ameaça | Descrição do Risco | Mitigação Implementada |
|---|---|---|
| **Prompt Injection Indireto** | Comandos maliciosos inseridos em logs, saídas de terminal ou arquivos para tentar enganar a IA. | • Isolamento de dados em `<untrusted>...</untrusted>`.<br>• Instruções de sistema de alta prioridade proibindo bypass.<br>• `PolicyEngine` reavalia todos os comandos gerados independentemente da confiança da IA. |
| **Bypass de Sudo Cacheado / Elevação Arbitrária** | Execução de comandos como root aproveitando sessão ativa do `sudo` ou scripts invisíveis. | • Bloqueio de `sudo`, `su`, `pkexec`, `doas` nas ferramentas padrão de shell.<br>• Elevação administrativa restrita ao helper `zashterminal-admin-helper` via Polkit.<br>• Validação estrita de parâmetros por expressões regulares. |
| **Exfiltração de Credenciais e Segredos** | O modelo de IA tentar ler arquivos sensíveis (`~/.ssh`, `~/.aws`, `.env`) e enviá-los a provedores em nuvem. | • `PathGuard` com denylist de leitura ativa para chaves SSH, GPG, AWS e `.env`.<br>• Redator automático de segredos (`redactor.py`) mascarando tokens, chaves RSA e senhas antes do envio. |
| **Persistência via Dotfiles** | Injeção de aliases maliciosos ou scripts de inicialização em `.bashrc`, `.zshrc`, `.profile`. | • `PathGuard` bloqueia escrita em dotfiles de shell e diretórios de inicialização.<br>• Edições de arquivos requerem staging e revisão visual de diff com opção de backup automático. |
| **Bypass por Links Simbólicos (Symlink Traversal)** | Criação de links simbólicos dentro de pastas permitidas apontando para `/etc/shadow` ou chaves privadas. | • `PathGuard` resolve o caminho canônico real (`os.path.realpath` / `Path.resolve()`) antes de validar qualquer política de acesso. |
| **Injeção de Metacaracteres Shell** | Injeção de operadores como `;`, `\|`, `&&`, `$(...)` em argumentos de comando. | • Vetorização obrigatória `argv: list[str]` com `shell=False`.<br>• O helper Polkit valida cada parâmetro com regex restrita (ex: `^[0-9]+[dwmy]$`). |

---

## 3. Estratificação de Níveis de Risco

O sistema categoriza todas as ações em 5 níveis de risco estritos:

- 🟢 **Nível 0 (Leitura Segura):** Comandos de diagnóstico e inspeção sem efeitos colaterais (`ls`, `df`, `free`, `uptime`, `ip route`, `journalctl --disk-usage`). Aprovados com 1 clique.
- 🔵 **Nível 1 (Escrita no Usuário):** Criação e edição de arquivos na home do usuário. Requer visualização de diff unificado e criação de backup prévio.
- 🟠 **Nível 2 (Administração do Sistema):** Tarefas administrativas pré-definidas em `admin_actions.json` (ex: limpeza de logs, manutenção de pacotes). Requer diálogo de autenticação gráfica do Polkit.
- 🔴 **Nível 3 (Ações Críticas):** Remoção de pacotes ou arquivos não-bloqueados. Requer confirmação explícita do usuário.
- ⛔ **Nível 4 (Bloqueado / Proibido):** Padrões perigosos e destrutivos (ex: `rm -rf /`, `mkfs.*`, `dd of=/dev/sd*`, `chmod 777 /`). Botão desabilitado na interface com bloqueio intransponível.

---

## 4. Auditoria e Rollback

1. **Trilha de Auditoria Append-Only:**
   - Todas as decisões do usuário, comandos avaliados e resultados de execução são registrados em `~/.local/share/zashterminal/audit/audit.jsonl`.
   - Política de retenção configurável com rotação automática.

2. **Mecanismo de Rollback Byte-Identical:**
   - Antes de aplicar qualquer arquivo colocado em staging, um backup do original é gravado em `~/.local/share/zashterminal/backups/` com checksum SHA-256 no manifesto.
   - O usuário pode reverter alterações através do diálogo de auditoria com integridade verificada.

---

## 5. Reportando Problemas de Segurança

Caso encontre uma vulnerabilidade de segurança no Zashterminal, por favor reporte diretamente através de uma issue de segurança ou pelo e-mail do mantenedor: `leo4berbert@gmail.com`.
