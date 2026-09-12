# tests/test_ai_assistant_script_filter.py
"""Unit tests for script detection and command extraction filters in AI Assistant and Planner."""

import unittest
from onyxsh.terminal.ai_assistant import TerminalAiAssistant
from onyxsh.agent.planner import (
    PlanParser,
    is_multi_line_script,
    is_valid_cli_command,
    split_command_to_argv,
)


class TestAiAssistantScriptFilter(unittest.TestCase):
    """Test suite for AI Assistant response command extraction and script filtering."""

    def setUp(self):
        self.assistant = TerminalAiAssistant.__new__(TerminalAiAssistant)

    def test_is_multi_line_script_detection(self):
        """Should recognize bash scripts with shebang, functions, and control structures."""
        script_sample = """#!/bin/bash
check_permissions() {
    if [ ! -w /etc/hosts ]; then
        echo "Permissão negada!"
        exit 1
    fi
}
menu() {
    case $choice in
        1)
            echo "Opção 1"
            ;;
        *)
            echo "Opção inválida"
    esac
}
check_permissions
menu
"""
        self.assertTrue(TerminalAiAssistant._is_multi_line_script(script_sample))
        self.assertTrue(is_multi_line_script(script_sample))

    def test_is_not_multi_line_script_for_cli_commands(self):
        """Should not classify simple list of CLI commands as a multi-line script."""
        commands_sample = """sudo apt update
sudo apt install -y nginx curl git
chmod +x script.sh
./script.sh
"""
        self.assertFalse(TerminalAiAssistant._is_multi_line_script(commands_sample))
        self.assertFalse(is_multi_line_script(commands_sample))

    def test_is_valid_cli_command(self):
        """Should accept valid CLI commands and reject syntax fragments / placeholders."""
        # Valid commands
        self.assertTrue(TerminalAiAssistant._is_valid_cli_command("sudo apt update"))
        self.assertTrue(TerminalAiAssistant._is_valid_cli_command("chmod +x bloqueio_hosts.sh"))
        self.assertTrue(TerminalAiAssistant._is_valid_cli_command("./bloqueio_hosts.sh"))
        self.assertTrue(TerminalAiAssistant._is_valid_cli_command("systemctl restart nginx"))

        # Invalid: Syntax keywords and structural tokens
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command("fi"))
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command(";;"))
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command("esac"))
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command("}"))
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command("1)"))
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command("*)"))
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command("check_permissions() {"))
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command("local ip=$1"))

        # Invalid: Documentation placeholders with angle brackets
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command("<endereço-ip> <nome-do-domínio>"))
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command("curl http://example.com -H 'Authorization: Bearer <token>'"))

        # Invalid: Comments and headers
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command("# Comentário de ajuda"))
        self.assertFalse(TerminalAiAssistant._is_valid_cli_command("===== Menu Principal ====="))

    def test_parse_assistant_payload_markdown_script_response(self):
        """When AI returns a full script in markdown, it should NOT split it into broken commands."""
        response_content = """Claro! Aqui está o script solicitado:

```bash
#!/bin/bash

check_permissions() {
    if [ ! -w /etc/hosts ]; then
        echo "Permissão negada!"
        exit 1
    fi
}

add_to_hosts() {
    local ip=$1
    local domain=$2
    echo "$ip $domain" | sudo tee -a /etc/hosts
}

menu() {
    echo "1. Adicionar IP"
    read choice
    case $choice in
        1)
            add_to_hosts "127.0.0.1" "teste.local"
            ;;
    esac
}

check_permissions
menu
```

### Como usar o Script:
```bash
chmod +x bloqueio_hosts.sh
./bloqueio_hosts.sh
```
"""
        reply, commands, code_snippets = self.assistant._parse_assistant_payload(response_content)

        # The script should be stored in code_snippets
        self.assertTrue(any("check_permissions" in s.get("code", "") for s in code_snippets))

        # Only genuine CLI commands should be extracted
        command_texts = [c["command"] for c in commands]
        self.assertIn("chmod +x bloqueio_hosts.sh", command_texts)
        self.assertIn("./bloqueio_hosts.sh", command_texts)

        # Broken script fragments MUST NOT be present in commands
        self.assertNotIn("fi", command_texts)
        self.assertNotIn(";;", command_texts)
        self.assertNotIn("esac", command_texts)
        self.assertNotIn("}", command_texts)
        self.assertNotIn("1)", command_texts)
        self.assertNotIn("check_permissions() {", command_texts)
        self.assertNotIn("local ip=$1", command_texts)

    def test_planner_markdown_script_parsing(self):
        """PlanParser should not generate ActionSteps for script syntax fragments."""
        script_markdown = """
        Aqui está o script de configuração:
        ```bash
        #!/bin/bash
        setup() {
            mkdir -p /tmp/mytest
            if [ -d /tmp/mytest ]; then
                echo "Criado"
            fi
        }
        setup
        ```
        Para testar:
        ```bash
        ls -la /tmp/mytest
        ```
        """
        plan = PlanParser.parse(script_markdown, provider_name="ollama")
        if hasattr(plan, "steps"):
            steps_cmd = [s.description for s in plan.steps]
            self.assertIn("ls -la /tmp/mytest", steps_cmd)
            self.assertNotIn("fi", steps_cmd)
            self.assertNotIn("setup() {", steps_cmd)

    def test_heredoc_placeholder_repair(self):
        """When AI provides a heredoc template with '...', it should inject the full script."""
        response_content = """Aqui está o script completo:

```bash
#!/bin/bash
echo "Script Completo e Funcional"
exit 0
```

Como usar:
```bash
cat << 'EOF' > ~/meuscript.sh
#!/bin/bash
... (inserir conteúdo do script aqui)
EOF
chmod +x ~/meuscript.sh
```
"""
        reply, commands, code_snippets = self.assistant._parse_assistant_payload(response_content)
        command_texts = [c["command"] for c in commands]
        
        # Verify the heredoc was repaired with the real script and not literal dots
        heredoc_cmd = next((c for c in command_texts if "<<" in c), None)
        self.assertIsNotNone(heredoc_cmd)
        self.assertIn('echo "Script Completo e Funcional"', heredoc_cmd)
        self.assertNotIn("...", heredoc_cmd)
        self.assertNotIn("(inserir conteúdo", heredoc_cmd)

    def test_script_creation_synthesis(self):
        """When AI provides a script and only chmod/run commands, synthesis should create the heredoc."""
        response_content = """Aqui está o script:

```bash
#!/bin/bash
echo "Iniciando backup"
tar -czf /tmp/backup.tar.gz ~/docs
```

Para rodar:
```bash
chmod +x ~/backup.sh
./backup.sh
```
"""
        reply, commands, code_snippets = self.assistant._parse_assistant_payload(response_content)
        command_texts = [c["command"] for c in commands]
        self.assertTrue(any("<<" in c for c in command_texts))
        self.assertIn("chmod +x ~/backup.sh", command_texts)

    def test_collapse_fragmented_echo_and_unclosed_heredoc(self):
        """When local LLM returns fragmented echo >> lines or unclosed heredoc in JSON, collapse it."""
        raw_json_response = """{
  "reply": "Aqui está o script:\\n\\n```bash\\n#!/usr/bin/env bash\\nadiciona_ip() {\\n    echo 'Adicionando IP'\\n}\\nadiciona_ip\\n```",
  "commands": [
    "cat << 'EOF' > ~/bloqueador_hosts.sh",
    "echo '#!/usr/bin/env bash' >> ~/bloqueador_hosts.sh",
    "echo '' >> ~/bloqueador_hosts.sh",
    "echo 'adiciona_ip() {' >> ~/bloqueador_hosts.sh",
    "echo '    echo \\'Adicionando IP\\'' >> ~/bloqueador_hosts.sh",
    "echo '}' >> ~/bloqueador_hosts.sh",
    "chmod +x ~/bloqueador_hosts.sh",
    "~/bloqueador_hosts.sh"
  ]
}"""
        reply, commands, code_snippets = self.assistant._parse_assistant_payload(raw_json_response)
        command_texts = [c["command"] for c in commands]

        # Should have collapsed the unclosed heredoc + 5 echo lines into 1 clean heredoc
        self.assertEqual(len(command_texts), 3)
        self.assertTrue(command_texts[0].startswith("cat << 'EOF' > ~/bloqueador_hosts.sh"))
        self.assertIn("adiciona_ip() {", command_texts[0])
        self.assertTrue(command_texts[0].endswith("EOF"))
        self.assertEqual(command_texts[1], "chmod +x ~/bloqueador_hosts.sh")
        self.assertEqual(command_texts[2], "~/bloqueador_hosts.sh")

    def test_auto_wrap_raw_unwrapped_script(self):
        """When local LLM emits raw bash code without ```bash code fence, OnyxSH should wrap and fix it."""
        raw_reply = """# Script de Bloqueio de IP/Hostname no Arquivo /etc/hosts

if [ "\\$(whoami)" != "root" ]; then
  echo "Este script precisa ser executado como root. Use sudo.
  exit 1
fi

adicionar_ao_hosts() {
  IP="\\$1"
  HOSTNAME="\\$2"
  echo -e "\\n\\$IP \\$HOSTNAME" | sudo tee --append /etc/hosts > /dev/null
}

while true; do
  read -p "Opcao: " OPCAO
  case \\$OPCAO in
    1)
      adicionar_ao_hosts
      ;;
  esac
done"""
        wrapped = TerminalAiAssistant._auto_wrap_raw_scripts_in_markdown(raw_reply)
        self.assertIn("```bash", wrapped)
        self.assertIn("$(whoami)", wrapped)
        self.assertNotIn(r"\$(whoami)", wrapped)
        # Should fix unclosed quote on echo statement
        self.assertIn('echo "Este script precisa ser executado como root. Use sudo."', wrapped)

    def test_split_command_to_argv_quoted_paths_with_spaces(self):
        """BUG-005: split_command_to_argv must preserve quoted paths with spaces as single argv elements."""
        cmd = 'mkdir -p "/home/user/Meus Documentos/Projeto"'
        argv = split_command_to_argv(cmd)
        self.assertEqual(argv, ["mkdir", "-p", "/home/user/Meus Documentos/Projeto"])

        cmd_git = 'git commit -m "feat(core): initial commit with multiple spaces"'
        argv_git = split_command_to_argv(cmd_git)
        self.assertEqual(argv_git, ["git", "commit", "-m", "feat(core): initial commit with multiple spaces"])

    def test_split_command_to_argv_heredocs(self):
        """BUG-005: Heredoc file creation commands should remain intact without breaking on lines."""
        heredoc = "cat << 'EOF' > ~/meu_script.sh\n#!/bin/bash\necho 'hello'\nEOF"
        argv = split_command_to_argv(heredoc)
        self.assertEqual(len(argv), 1)
        self.assertEqual(argv[0], heredoc)

    def test_planner_preserves_quoted_arguments_in_argv(self):
        """BUG-005: PlanParser should generate ActionStep with clean uncorrupted argv for paths with spaces."""
        markdown_resp = """
        Para criar a pasta e o arquivo:
        ```bash
        mkdir -p "/tmp/pasta com espaco"
        touch "/tmp/pasta com espaco/arquivo novo.txt"
        ```
        """
        plan = PlanParser.parse(markdown_resp, provider_name="ollama")
        self.assertTrue(hasattr(plan, "steps"))
        self.assertEqual(len(plan.steps), 2)
        self.assertEqual(plan.steps[0].argv, ["mkdir", "-p", "/tmp/pasta com espaco"])
        self.assertEqual(plan.steps[1].argv, ["touch", "/tmp/pasta com espaco/arquivo novo.txt"])


    def test_clean_overescaped_command_fixes_escaped_apostrophes_and_duplicate_tokens(self):
        """Should clean up corrupted escapes and token repetitions in commands from local LLMs."""
        # Exact issue reported by user
        cmd_corrupted = r'cd "$HOME/Dante\'s \'s Inferno PC PORT"'
        cleaned = TerminalAiAssistant._clean_overescaped_command(cmd_corrupted)
        self.assertEqual(cleaned, 'cd "$HOME/Dante\'s Inferno PC PORT"')

        # Double escaped slash
        cmd_double_escaped = 'cd "$HOME/Dante\\\\\'s \'s Inferno PC PORT"'
        cleaned_double = TerminalAiAssistant._clean_overescaped_command(cmd_double_escaped)
        self.assertEqual(cleaned_double, 'cd "$HOME/Dante\'s Inferno PC PORT"')

        # Relative path with corrupted escaping
        cmd_rel = r'cd "Dante\'s \'s Inferno PC PORT"'
        self.assertEqual(
            TerminalAiAssistant._clean_overescaped_command(cmd_rel),
            'cd "Dante\'s Inferno PC PORT"'
        )

        # Doubled double quotes
        cmd_quotes = 'cd ""$HOME/Dante""'
        self.assertEqual(
            TerminalAiAssistant._clean_overescaped_command(cmd_quotes),
            'cd "$HOME/Dante"'
        )

        # Distorted ./~/ path
        cmd_path = './~/scripts/run.sh'
        self.assertEqual(
            TerminalAiAssistant._clean_overescaped_command(cmd_path),
            '~/scripts/run.sh'
        )

        # Valid unquoted escaped command should be preserved
        cmd_valid_unquoted = r"cd Dante\'s\ Inferno\ PC\ PORT"
        self.assertEqual(
            TerminalAiAssistant._clean_overescaped_command(cmd_valid_unquoted),
            cmd_valid_unquoted
        )

    def test_clean_overescaped_reply_text(self):
        """Should clean over-escaped patterns in explanatory text and markdown blocks."""
        reply = (
            "Para abrir a pasta, use:\n\n"
            '`cd "$HOME/Dante\\\'s \'s Inferno PC PORT"`\n\n'
            "```bash\n"
            'cd "$HOME/Dante\\\'s \'s Inferno PC PORT"\n'
            "```"
        )
        cleaned = TerminalAiAssistant._clean_overescaped_reply_text(reply)
        self.assertNotIn(r"\'s \'s", cleaned)
        self.assertNotIn(r"\\'s", cleaned)
        self.assertIn('`cd "$HOME/Dante\'s Inferno PC PORT"`', cleaned)
        self.assertIn('cd "$HOME/Dante\'s Inferno PC PORT"', cleaned)

    def test_system_prompt_includes_cwd_context_and_clean_quoting(self):
        """System prompt should include clean quoting rules and cwd context when available."""
        prompt_with_cwd = TerminalAiAssistant._get_system_prompt(cwd="~/Projetos")
        self.assertIn("CURRENT WORKING DIRECTORY", prompt_with_cwd)
        self.assertIn("~/Projetos", prompt_with_cwd)
        self.assertIn("DYNAMIC PATHS & CLEAN COMMAND QUOTING (KISS)", prompt_with_cwd)
        self.assertIn("Dante's Inferno PC PORT", prompt_with_cwd)

        prompt_without_cwd = TerminalAiAssistant._get_system_prompt()
        self.assertNotIn("CURRENT WORKING DIRECTORY", prompt_without_cwd)
        self.assertIn("DYNAMIC PATHS & CLEAN COMMAND QUOTING (KISS)", prompt_without_cwd)

    def test_get_current_working_directory_from_terminal(self):
        """get_current_working_directory should sanitize home directory and subpaths."""
        import pathlib
        from unittest.mock import MagicMock

        home = str(pathlib.Path.home()).rstrip("/")

        # Mock terminal at $HOME
        term_home = MagicMock()
        term_home.get_current_directory_uri.return_value = f"file://{home}"
        self.assistant.terminal_manager = MagicMock()
        self.assistant.terminal_manager.get_active_terminal.return_value = term_home
        self.assertEqual(self.assistant.get_current_working_directory(), "~")

        # Mock terminal in subfolder of $HOME
        term_sub = MagicMock()
        term_sub.get_current_directory_uri.return_value = f"file://{home}/Projetos/OnyxSH"
        self.assistant.terminal_manager.get_active_terminal.return_value = term_sub
        self.assertEqual(self.assistant.get_current_working_directory(), "~/Projetos/OnyxSH")

        # Mock terminal in system directory
        term_sys = MagicMock()
        term_sys.get_current_directory_uri.return_value = "file:///etc/nginx"
        self.assistant.terminal_manager.get_active_terminal.return_value = term_sys
        self.assertEqual(self.assistant.get_current_working_directory(), "/etc/nginx")


if __name__ == "__main__":
    unittest.main()
