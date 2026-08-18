# tests/test_tunnel_manager.py
import unittest
from onyxsh.sessions.models import SessionItem, SessionValidationError
from onyxsh.terminal.tunnel_manager import SSHTunnel, SSHTunnelManager, get_ssh_tunnel_manager


class TestSSHTunnelManager(unittest.TestCase):
    def setUp(self):
        self.session = SessionItem(
            name="Production Server",
            session_type="ssh",
            host="192.168.1.100",
            user="admin",
            port=22,
        )

    def test_local_port_forward_normalization(self):
        """Test normalization and validation of Local (-L) port forwarding."""
        tunnel_dict = {
            "type": "local",
            "name": "Database Tunnel",
            "local_host": "127.0.0.1",
            "local_port": 5432,
            "remote_host": "db.internal",
            "remote_port": 5432,
            "auto_start": True,
        }
        self.session.port_forwardings = [tunnel_dict]
        self.assertEqual(len(self.session.port_forwardings), 1)
        res = self.session.port_forwardings[0]
        self.assertEqual(res["type"], "local")
        self.assertEqual(res["name"], "Database Tunnel")
        self.assertEqual(res["local_port"], 5432)
        self.assertEqual(res["remote_host"], "db.internal")
        self.assertEqual(res["remote_port"], 5432)
        self.assertTrue(res["auto_start"])
        self.assertEqual(len(self.session.get_validation_errors()), 0)

    def test_remote_port_forward_normalization(self):
        """Test normalization and validation of Remote (-R) port forwarding."""
        tunnel_dict = {
            "type": "remote",
            "name": "Webhook Exposer",
            "local_host": "localhost",
            "local_port": 3000,
            "remote_host": "0.0.0.0",
            "remote_port": 8080,
        }
        self.session.port_forwardings = [tunnel_dict]
        res = self.session.port_forwardings[0]
        self.assertEqual(res["type"], "remote")
        self.assertEqual(res["remote_port"], 8080)
        self.assertEqual(len(self.session.get_validation_errors()), 0)

    def test_dynamic_socks5_forward_normalization(self):
        """Test normalization and validation of Dynamic SOCKS5 (-D) port forwarding."""
        tunnel_dict = {
            "type": "dynamic",
            "name": "Corporate Proxy",
            "local_host": "127.0.0.1",
            "local_port": 1080,
        }
        self.session.port_forwardings = [tunnel_dict]
        res = self.session.port_forwardings[0]
        self.assertEqual(res["type"], "dynamic")
        self.assertEqual(res["local_port"], 1080)
        self.assertEqual(res["remote_port"], 0)
        # Dynamic forwarding does not require remote_host / remote_port
        self.assertEqual(len(self.session.get_validation_errors()), 0)

    def test_invalid_port_range_validation(self):
        """Test that invalid ports are caught by get_validation_errors."""
        invalid_tunnel = {
            "type": "local",
            "name": "Bad Port",
            "local_host": "127.0.0.1",
            "local_port": 80,  # Below 1025 for non-root local binding
            "remote_host": "example.com",
            "remote_port": 80,
        }
        self.session.port_forwardings = [invalid_tunnel]
        errors = self.session.get_validation_errors()
        self.assertTrue(len(errors) > 0)
        self.assertTrue(any("1025" in err for err in errors))

    def test_ssh_tunnel_dataclass_methods(self):
        """Test display formatting in SSHTunnel data class."""
        t_local = SSHTunnel(
            name="Postgres",
            type="local",
            local_host="127.0.0.1",
            local_port=5432,
            remote_host="db.internal",
            remote_port=5432,
        )
        self.assertEqual(t_local.get_display_source(), "127.0.0.1:5432")
        self.assertEqual(t_local.get_display_target(), "db.internal:5432")
        self.assertIn("Local", t_local.get_type_label())

        t_dynamic = SSHTunnel(
            name="SOCKS",
            type="dynamic",
            local_host="127.0.0.1",
            local_port=1080,
        )
        self.assertEqual(t_dynamic.get_display_source(), "127.0.0.1:1080")
        self.assertEqual(t_dynamic.get_display_target(), "SOCKS5 Proxy")
        self.assertIn("Dynamic", t_dynamic.get_type_label())

        t_remote = SSHTunnel(
            name="Remote Web",
            type="remote",
            local_host="localhost",
            local_port=3000,
            remote_host="0.0.0.0",
            remote_port=9000,
        )
        self.assertEqual(t_remote.get_display_source(), "remote:9000")
        self.assertEqual(t_remote.get_display_target(), "localhost:3000")

    def test_tunnel_manager_lifecycle(self):
        """Test registration, lookup and unregistration in SSHTunnelManager."""
        manager = SSHTunnelManager()
        tunnel = SSHTunnel(
            name="Test Tunnel",
            session_name="Test Host",
            type="local",
            local_port=8888,
            remote_host="target.com",
            remote_port=80,
        )
        tid = manager.register_tunnel(tunnel)
        self.assertEqual(tid, tunnel.id)

        retrieved = manager.get_tunnel(tid)
        self.assertIsNotNone(retrieved)
        self.assertEqual(retrieved.name, "Test Tunnel")

        all_tunnels = manager.get_all_tunnels()
        self.assertEqual(len(all_tunnels), 1)

        unreg = manager.unregister_tunnel(tid)
        self.assertTrue(unreg)
        self.assertIsNone(manager.get_tunnel(tid))


if __name__ == "__main__":
    unittest.main()
