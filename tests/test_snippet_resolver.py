# tests/test_snippet_resolver.py
import unittest
from onyxsh.data.snippet_resolver import (
    SnippetContextResolver,
    SnippetVariable,
    get_snippet_resolver,
)
from onyxsh.data.command_manager_models import (
    CommandButton,
    ExecutionMode,
    DisplayMode,
)


class TestSnippetResolver(unittest.TestCase):
    def setUp(self):
        self.resolver = get_snippet_resolver()

    def test_extract_variables_double_brackets(self):
        template = "docker logs -f {{container}} --tail {{lines=100}}"
        vars_list = self.resolver.extract_variables(template)
        self.assertEqual(len(vars_list), 2)
        self.assertEqual(vars_list[0].name, "container")
        self.assertEqual(vars_list[0].default_value, "")
        self.assertFalse(vars_list[0].is_system)

        self.assertEqual(vars_list[1].name, "lines")
        self.assertEqual(vars_list[1].default_value, "100")
        self.assertFalse(vars_list[1].is_system)

    def test_extract_system_variables(self):
        template = "rsync -avz {{cwd}} {{user}}@{{host}}:{{dest=/backup}}"
        vars_list = self.resolver.extract_variables(template)
        self.assertEqual(len(vars_list), 4)

        cwd_var = next(v for v in vars_list if v.name == "cwd")
        self.assertTrue(cwd_var.is_system)

        dest_var = next(v for v in vars_list if v.name == "dest")
        self.assertFalse(dest_var.is_system)
        self.assertEqual(dest_var.default_value, "/backup")

    def test_get_custom_variables(self):
        template = "git commit -m '{{message}}' && git push {{host}} {{git_branch}}"
        custom = self.resolver.get_custom_variables(template)
        self.assertEqual(len(custom), 1)
        self.assertEqual(custom[0].name, "message")

    def test_resolve_template_with_user_values(self):
        template = "docker logs -f {{container}} --tail {{lines=100}}"
        resolved = self.resolver.resolve_template(
            template, user_values={"container": "web_api", "lines": "250"}
        )
        self.assertEqual(resolved, "docker logs -f web_api --tail 250")

    def test_resolve_template_fallback_to_defaults(self):
        template = "docker logs -f {{container=my_app}} --tail {{lines=50}}"
        resolved = self.resolver.resolve_template(
            template, user_values={"container": "nginx"}
        )
        self.assertEqual(resolved, "docker logs -f nginx --tail 50")

    def test_resolve_system_context(self):
        template = "echo Today is {{date}} at host {{host}}"
        resolved = self.resolver.resolve_template(template)
        self.assertIn("Today is 20", resolved)
        self.assertIn("at host localhost", resolved)

    def test_command_button_snippet_integration(self):
        btn = CommandButton(
            id="test_btn",
            name="Test Logs",
            description="Test container logs",
            command_template="docker logs {{container}} --tail {{lines=50}}",
        )
        self.assertTrue(btn.has_custom_variables())
        vars_found = btn.parse_snippet_variables()
        self.assertEqual(len(vars_found), 2)

        built = btn.build_command({"container": "redis", "lines": "10"})
        self.assertEqual(built, "docker logs redis --tail 10")


if __name__ == "__main__":
    unittest.main()
