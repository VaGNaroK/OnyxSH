# 🧠 OnyxSH — Guia de Referência para Agentes de IA

> **Versão auditada:** v0.9.0 → HEAD (Agosto 2025)
> **Última atualização:** 2025-08-25
> **Objetivo:** Documento de referência para agentes de IA que trabalham neste projeto, contendo bugs conhecidos, melhorias sugeridas, padrões arquiteturais e armadilhas a evitar.

---

## 📐 Visão Arquitetural

### Módulos Principais

| Módulo | Caminho | Responsabilidade |
|--------|---------|-----------------|
| **AI Assistant** | `src/onyxsh/terminal/ai_assistant.py` | Orquestração de LLM, sanitização de comandos, integração terminal ↔ IA |
| **Agent Mode** | `src/onyxsh/agent/` | Motor seguro de agente: planner, orchestrator, policy engine, verifier |
| **Smart Router** | `src/onyxsh/agent/router.py` | Roteamento inteligente entre modelos rápidos/avançados |
| **Semantic Tracker** | `src/onyxsh/terminal/semantic_tracker.py` | Rastreamento OSC 133 de prompts, comandos e saídas |
| **Production Guard** | `src/onyxsh/terminal/production_guard.py` | Detecção de comandos destrutivos em terminais de produção |
| **File Manager** | `src/onyxsh/filemanager/` | Gerenciador de arquivos integrado com Quick Look |
| **Sessions** | `src/onyxsh/sessions/` | Gerenciamento e persistência de sessões SSH/locais |
| **Settings** | `src/onyxsh/settings/manager.py` | Configurações centralizadas com UI |
| **Git Utils** | `src/onyxsh/utils/git_utils.py` | Inspeção Git, staging, auditoria de secrets |
| **Security** | `src/onyxsh/utils/security.py` | Validação de hostname, SSH keys, path sanitization |
| **Logger** | `src/onyxsh/utils/logger.py` | Sistema de logging thread-safe com rotação |
| **Task Manager** | `src/onyxsh/core/tasks.py` | Thread pools singleton para I/O e CPU |

### Padrões de Design Utilizados

- **Singleton (thread-safe):** `LoggerManager`, `AsyncTaskManager`, `SemanticTracker`, `ProductionGuard`, `AuditLogger`
- **WeakKeyDictionary:** `SemanticTracker._terminals` para auto-cleanup de terminais destruídos
- **Policy Engine + Path Guard:** Camada dupla de segurança para o agente IA
- **Redactor:** Filtragem de secrets antes de envio a LLMs remotos
- **Flatpak-aware:** Todas as chamadas subprocess verificam `is_flatpak_sandbox()` e usam `flatpak-spawn --host` quando necessário

---

## 🐛 Bugs Confirmados

### ~~BUG-001: `locale.getdefaultlocale()` Deprecado (Python 3.11+)~~ ✅ *[RESOLVIDO - Commit fee81a1]*

**Severidade:** ⚠️ Média — Gera `DeprecationWarning` em Python 3.11+ e será removido em versões futuras.

**Arquivos afetados:**
- `src/onyxsh/terminal/ai_assistant.py:105`
- `src/onyxsh/utils/platform.py:131`

**Status:** Corrigido utilizando `locale.getlocale()` com fallback via `os.environ["LANG"]`.

---

### ~~BUG-002: Redactor Não Detecta Secrets em Hex/Base64 Genéricos~~ ✅ *[RESOLVIDO - Commit 2b0c7e2]*

**Severidade:** ⚠️ Média — Tokens hexadecimais longos (e.g., tokens de deploy do GitLab, Vercel, Netlify) passam sem redação.

**Arquivo:** `src/onyxsh/agent/redactor.py`

**Status:** Corrigido adicionando detecção de GitLab PAT (`glpat-*`), Slack (`xoxb-*`, `xoxp-*`), Vercel (`vercel_*`), HashiCorp Vault (`hvs.*`), migrado para `re.subn()` para contagem precisa e proteção contra re-redação de placeholders. Coberto por novos testes em `tests/test_redactor.py`.

---

### ~~BUG-003: Production Guard — Bypass via Subshell e Variáveis~~ ✅ *[RESOLVIDO - Commit c463a3f]*

**Severidade:** 🔴 Alta — Comandos destrutivos podiam evadir detecção quando encapsulados em construções shell.

**Arquivo:** `src/onyxsh/terminal/production_guard.py`

**Status:** Corrigido adicionando extração e desaninhamento recursivo de subshells (`bash -c`, `sh -c`, etc.), `eval`, variáveis com comandos embutidos, remoção de wrappers estendidos (`exec`, `builtin`, `command`, `xargs`), detecção de comandos perigosos com `xargs rm`, além de regras dedicadas para pipes para interpretadores (`| bash`) e pipelines de decodificação base64. Coberto por 5 novos métodos de teste em `tests/test_production_guard.py`.

---

### ~~BUG-004: Logger — FileHandler Leak em `reconfigure_all_loggers()`~~ ✅ *[RESOLVIDO - Commit 3d18ce0]*

**Severidade:** ⚠️ Média — Resource leak ao reconfigurar loggers repetidamente.

**Arquivo:** `src/onyxsh/utils/logger.py:176`

**Status:** Corrigido adicionando flush e `.close()` explícito em cada handler antes de desanexar em `_setup_logger()`. Também implementados métodos `close()` em `ThreadSafeLogger` e `close_all_loggers()` em `LoggerManager`. Coberto por nova suíte de testes em `tests/test_logger.py`.

---

### ~~BUG-005: Planner — `argv` Split Ingênuo Quebra Heredocs e Caminhos com Espaços~~ ✅ *[RESOLVIDO - Commit c5d0171]*

**Severidade:** 🔴 Alta — Comandos com espaços em paths ou heredocs eram corrompidos pelo `.split()` simples.

**Arquivo:** `src/onyxsh/agent/planner.py`

**Status:** Corrigido implementando `split_command_to_argv(cmd_str)` utilizando `shlex.split(posix=True)` com preservação de heredocs (`<<`) como strings intactas e fallback para parsing seguro. Coberto por testes unitários em `tests/test_ai_assistant_script_filter.py`.

---

### ~~BUG-006: AuditLogger — `rotate()` Não é Atomic em Cenários de I/O Lento~~ ✅ *[RESOLVIDO - Commit 735500c]*

**Severidade:** ⚠️ Baixa — Risco de corrupção caso houvesse falha ou interrupção durante reescrita direta.

**Arquivo:** `src/onyxsh/agent/audit.py:86`

**Status:** Corrigido reescrevendo em arquivo temporário com `fsync` seguido de `replace()` atômico no mesmo filesystem com cleanup em caso de exceção. Coberto por testes unitários em `tests/test_audit_rollback.py`.

---

### ~~BUG-007: `safe_quote_path()` — Injeção via `$HOME` em Subpath~~ ✅ *[RESOLVIDO - Commit 426dca9]*

**Severidade:** ⚠️ Média — Subpaths de `~/` e `$HOME/` contendo metacaracteres shell podiam ser expandidos de forma insegura.

**Arquivo:** `src/onyxsh/agent/verifier.py:66-85`

**Status:** Corrigido com verificação de metacaracteres shell perigosos (`$`, `` ` ``, `!`, `\`, `"`, `\n`, `\r`, `\t`, `|`, `;`, `&`, `<`, `>`, `(`, `)`) e aplicação de `shlex.quote()` nos subpaths afetados. Coberto por testes unitários em `tests/test_post_verification.py`.

---

### ~~BUG-008: `HostnameValidator.resolve_hostname()` — SIGALRM Não Funciona em Threads~~ ✅ *[RESOLVIDO]*

**Severidade:** ⚠️ Média — `signal.setitimer(SIGALRM)` falhava com `ValueError` caso invocado fora da thread principal.

**Arquivo:** `src/onyxsh/utils/security.py:95-135`

**Status:** Corrigido substituindo o mecanismo de sinais (`SIGALRM`) por resolução assíncrona em worker thread com `thread.join(timeout=timeout)`. Coberto por nova suíte de testes em `tests/test_security.py`.

---

### BUG-009: `AsyncTaskManager` — `pending_io_tasks` / `pending_cpu_tasks` Nunca Funcionam

**Severidade:** ⚠️ Baixa — As properties verificam `_thread_name_prefix` no Future, mas `Future` não possui esse atributo.

**Arquivo:** `src/onyxsh/core/tasks.py:206-217`

**Código problemático:**
```python
return sum(1 for f in self._active_futures
          if not f.done() and "io" in str(getattr(f, '_thread_name_prefix', '')))
```

**Problema:** `concurrent.futures.Future` não tem `_thread_name_prefix`. O atributo pertence ao `ThreadPoolExecutor`, não ao `Future`.

**Correção recomendada:** Usar sets separados para rastrear futures de IO vs CPU:
```python
self._io_futures: Set[Future] = set()
self._cpu_futures: Set[Future] = set()
```

---

## 🔧 Melhorias Sugeridas

### IMP-001: Policy Engine — Frozenset com Multi-word Commands Ineficaz

**Arquivo:** `src/onyxsh/agent/policy_engine.py:14-29`

**Problema:** `READ_ONLY_COMMANDS` e `USER_WRITE_COMMANDS` contêm entradas multi-palavra como `"git status"`, `"git log"`, mas a busca em `classify()` usa `full_2cmd = f"{argv[0]} {argv[1]}"` que pode não corresponder quando há flags intermediárias.

**Exemplo falho:**
```python
# "git --no-pager log" → full_2cmd = "git --no-pager" → NÃO match "git log"
```

**Sugestão:** Construir um lookup baseado em prefixo:
```python
_READ_ONLY_GIT = {"status", "log", "diff", "show", "branch"}
if base_cmd == "git" and len(argv) > 1:
    for arg in argv[1:]:
        if not arg.startswith("-"):
            if arg in _READ_ONLY_GIT:
                return RiskLevel.READ_ONLY
            break
```

---

### IMP-002: Redactor — Contagem de Redações Imprecisa

**Arquivo:** `src/onyxsh/agent/redactor.py:72-76`

**Problema:** `pattern.findall()` conta matches **antes** da substituição, mas se um pattern anterior já alterou o texto, a contagem pode ser imprecisa.

**Sugestão:** Usar `re.subn()`:
```python
for pattern, replacement in SECRET_PATTERNS:
    redacted_text, n = pattern.subn(replacement, redacted_text)
    total_redactions += n
```

---

### IMP-003: Verifier — Suporte a `systemctl daemon-reload`

**Arquivo:** `src/onyxsh/agent/verifier.py:139-183`

**Ausência:** Quando um unit file é editado, o verifier deveria sugerir `systemctl daemon-reload` como passo de verificação antes de `start/restart`.

---

### IMP-004: Context Manager — `wrap_untrusted()` Não Escapa Tags XML

**Arquivo:** `src/onyxsh/agent/context_manager.py:68-72`

**Problema:** Se o conteúdo do terminal contiver `</untrusted>`, a boundary tag é quebrada, permitindo potencial prompt injection.

**Correção recomendada:**
```python
def wrap_untrusted(self, content: str, source: str = "terminal") -> str:
    if not content:
        return ""
    safe_content = content.strip().replace("</untrusted>", "&lt;/untrusted&gt;")
    return f'<untrusted source="{source}">\n{safe_content}\n</untrusted>'
```

---

### IMP-005: Smart Router — Log Expõe Preview de API Key

**Arquivo:** `src/onyxsh/agent/router.py:297`

**Problema:** O log imprime os primeiros 4 caracteres da API key. Em provedores com prefixos conhecidos (`sk-`, `gsk_`, `AIza`), isso confirma o tipo de chave.

**Sugestão:** Logar apenas comprimento e presença:
```python
key_info = f"present (len={len(decision.api_key)})" if decision.api_key else "NONE"
```

---

### IMP-006: `fs_tools.py` — `metadata()` Usa `datetime.fromtimestamp()` sem Timezone

**Arquivo:** `src/onyxsh/agent/fs_tools.py:150-151`

**Correção:**
```python
from datetime import datetime, timezone
"modified": datetime.fromtimestamp(st.st_mtime, tz=timezone.utc).isoformat(),
"created": datetime.fromtimestamp(st.st_ctime, tz=timezone.utc).isoformat(),
```

---

### IMP-007: `_staging_path` — Colisão de Nomes em Planos Simultâneos

**Arquivo:** `src/onyxsh/agent/fs_tools.py:26-31`

**Problema:** `_get_staging_path()` usa apenas `target_path.name` + `plan_id`, mas se dois steps editam arquivos com mesmo nome em diretórios diferentes, o staging colide.

**Sugestão:** Incluir hash do path completo:
```python
import hashlib
path_hash = hashlib.sha256(str(target_path).encode()).hexdigest()[:8]
sanitized_name = f"{target_path.stem}_{path_hash}{target_path.suffix}"
```

---

### IMP-008: `SemanticTerminalState` — Usar `deque` para `prompt_rows`

**Arquivo:** `src/onyxsh/terminal/semantic_tracker.py:73-74`

**Observação:** O cap é 200 entradas via `pop(0)` (O(n)). `deque(maxlen=200)` seria O(1):
```python
from collections import deque
self.prompt_rows: deque[int] = deque(maxlen=200)
```

---

### IMP-009: `git_utils.py` — `run_git_command()` Fallback Redundante

**Arquivo:** `src/onyxsh/utils/git_utils.py:113-121`

**Problema:** Se `shutil.which("git")` retorna `None` no fallback #1, o fallback #3 repete exatamente o mesmo `subprocess.run(["git"] + args)` que vai falhar.

**Sugestão:** Verificar `shutil.which("git")` antes do fallback #3 e retornar `CompletedProcess(returncode=127)` se git não está disponível.

---

### IMP-010: PathGuard — `/tmp` Symlink Attack Vector

**Arquivo:** `src/onyxsh/agent/path_guard.py:161-168, 194`

**Problema:** `can_write()` permite escrita em `/tmp`, o que poderia ser usado para criar symlinks estratégicos apontando para paths sensíveis.

**Sugestão:** Restringir `can_write()` em `/tmp` a não permitir criação de symlinks (verificar via `os.path.islink()` no tool layer).

---

## 🧪 Gaps na Cobertura de Testes

### Testes Inexistentes ou Incompletos

| Módulo | Arquivo de Teste | Status | Gap |
|--------|-----------------|--------|-----|
| `AuditLogger.rotate()` | `test_audit_rollback.py` | ⚠️ Parcial | Não testa cenário de falha de I/O |
| `ProductionGuard` | `test_production_guard.py` | ⚠️ Parcial | Não cobre subshell bypasses |
| `Redactor` | `test_redactor.py` | ⚠️ Parcial | Não testa tokens GitLab, Slack |
| `PlanParser.parse()` | `test_ai_assistant_script_filter.py` | ⚠️ Parcial | Não testa heredoc com espaços em paths |
| `safe_quote_path()` | `test_post_verification.py` | ⚠️ Parcial | Não testa metacaracteres shell |
| `ContextManager.wrap_untrusted()` | `test_context_manager.py` | ⚠️ Parcial | Não testa `</untrusted>` injection |
| `HostnameValidator.resolve_hostname()` | — | ❌ Ausente | Nenhum teste existe |
| `AsyncTaskManager` | — | ❌ Ausente | Nenhum teste para pending tasks |
| `SmartRouter.resolve_route()` | `test_smart_router.py` | ✅ Bom | Cobertura boa |
| `PolicyEngine.classify()` | `test_policy_engine.py` | ⚠️ Parcial | Não testa git com flags intermediárias |

### Testes Recomendados (Prioridade)

1. **`test_production_guard.py`** — Adicionar testes para `bash -c`, `eval`, `pipe to shell`
2. **`test_redactor.py`** — Adicionar testes para tokens GitLab, Slack, hex genéricos
3. **`test_logger.py`** (NOVO) — Verificar que handlers são fechados em reconfiguração
4. **`test_context_manager.py`** — Testar `wrap_untrusted()` com conteúdo contendo `</untrusted>`
5. **`test_async_task_manager.py`** (NOVO) — Testar `pending_io_tasks` / `pending_cpu_tasks`

---

## ⚡ Armadilhas Comuns para Desenvolvedores

### 1. Flatpak vs Nativo
Toda invocação de `subprocess.run()` que acessa o host **deve** verificar `is_flatpak_sandbox()` e usar `flatpak-spawn --host`.

### 2. Thread Safety no GLib
Callbacks do `SemanticTracker` são disparados via `GLib.idle_add()`. **Nunca** acessar widgets GTK diretamente de worker threads.

### 3. Policy Engine é a Última Linha de Defesa
O `PolicyEngine` **sempre** reclassifica o risco, mesmo que o LLM declare `risk: 0`.

### 4. Singleton Reset em Testes
`AsyncTaskManager.reset()` é necessário entre testes para evitar state leaking.

### 5. Redação de Secrets é Bidirecional
- **Input:** `ContextManager.process_attachment()` redacta antes de enviar ao LLM
- **Diff audit:** `git_utils.audit_diff_for_secrets()` analisa linhas `+` no diff staged

---

## 📋 Checklist para Novos Features

Ao implementar uma nova funcionalidade no OnyxSH:

- [ ] **Flatpak:** Se usa subprocess, funciona dentro de sandbox?
- [ ] **PathGuard:** Se acessa filesystem via agente, o path está dentro dos allowed roots?
- [ ] **PolicyEngine:** Se é um novo tool, está registrado no `ToolRegistry` com risk correto?
- [ ] **Redactor:** Se texto pode conter secrets, passa pelo `redact_secrets()` antes de logging/LLM?
- [ ] **Testes:** Cobertura em `tests/` com `unittest`?
- [ ] **i18n:** Strings de UI usam `_()` de `translation_utils`?
- [ ] **Logging:** Usa `get_logger()` em vez de `print()`?
- [ ] **Thread safety:** Acesso a dados compartilhados protegido por lock?
- [ ] **Singleton cleanup:** Se usa singletons, eles têm `.reset()` para testes?

---

## 📁 Estrutura de Testes

```
tests/
├── test_admin_helper.py          # AdminHelper polkit integration
├── test_ai_assistant_script_filter.py  # LLM output command extraction
├── test_ai_export.py             # Chat export functionality
├── test_ai_offline_mode.py       # Offline/local model routing
├── test_audit_rollback.py        # Audit log rotation
├── test_checksum.py              # File checksum utilities
├── test_command_history.py       # Terminal command history
├── test_command_palette.py       # Command palette search
├── test_completion_engine.py     # Tab completion engine
├── test_completion_specs.py      # Completion spec definitions
├── test_context_manager.py       # Prompt engineering & attachments
├── test_desktop_notifier.py      # Desktop notifications
├── test_diagnostics.py           # System diagnostics
├── test_filemanager_ai.py        # File manager AI integration
├── test_filemanager_bookmarks.py # File manager bookmarks
├── test_filemanager_status_bar.py # File manager status bar
├── test_fs_tools.py              # Filesystem tools (read, write, list)
├── test_git_assistant.py         # Git assistant (commit, diff)
├── test_icon_theme.py            # Icon resolution
├── test_llm_lifecycle.py         # LLM provider lifecycle
├── test_models.py                # ActionStep, ActionPlan models
├── test_path_guard.py            # PathGuard read/write policies
├── test_plan_execution.py        # Full plan execution flow
├── test_policy_engine.py         # Risk classification engine
├── test_post_verification.py     # PostVerifier inference & execution
├── test_production_guard.py      # Destructive command detection
├── test_quick_look.py            # Quick Look file preview
├── test_redactor.py              # Secret redaction
├── test_semantic_prompts.py      # OSC 133 semantic tracking
├── test_session_restore.py       # Session state save/restore
├── test_shell_tools.py           # Shell command execution tools
├── test_smart_router.py          # Smart model routing
├── test_snippet_resolver.py      # Snippet resolution
├── test_terminal_exporter.py     # Terminal content export
└── test_tunnel_manager.py        # SSH tunnel management
```

**Comando para executar todos os testes:**
```bash
PYTHONPATH=src python3 -m unittest discover -s tests
```

---

> **Nota para agentes de IA:** Este documento deve ser atualizado sempre que um bug listado for corrigido ou uma nova melhoria for identificada. Marque items resolvidos com ~~strikethrough~~ e adicione a data de resolução.
