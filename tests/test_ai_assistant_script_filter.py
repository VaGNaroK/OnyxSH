# tests/test_ai_assistant_script_filter.py
"""Unit tests for script detection and command extraction filters in AI Assistant and Planner."""

import unittest
from onyxsh.terminal.ai_assistant import TerminalAiAssistant
from onyxsh.agent.planner import PlanParser, is_multi_line_script, is_valid_cli_command


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


if __name__ == "__main__":
    unittest.main()
