"""Unit tests for AsyncTaskManager and background task pools (BUG-009)."""

import threading
import unittest
from onyxsh.core.tasks import AsyncTaskManager, submit_cpu, submit_io


class TestAsyncTaskManager(unittest.TestCase):
    def setUp(self):
        AsyncTaskManager.reset()
        self.manager = AsyncTaskManager.get()

    def tearDown(self):
        AsyncTaskManager.reset()

    def test_singleton(self):
        m1 = AsyncTaskManager.get()
        m2 = AsyncTaskManager.get()
        self.assertIs(m1, m2)

    def test_submit_io_and_pending_count(self):
        """BUG-009: pending_io_tasks must accurately count active IO tasks."""
        started = threading.Event()
        proceed = threading.Event()

        def sync_io_task():
            started.set()
            proceed.wait(timeout=2.0)
            return "io_done"

        future = self.manager.submit_io(sync_io_task)
        self.assertIsNotNone(future)

        # Wait until task has actually started executing
        started.wait(timeout=2.0)
        self.assertGreaterEqual(self.manager.pending_io_tasks, 1)
        self.assertEqual(self.manager.pending_cpu_tasks, 0)

        # Release task and verify result
        proceed.set()
        result = future.result(timeout=2.0)
        self.assertEqual(result, "io_done")

        # Upon completion, count is 0
        self.assertEqual(self.manager.pending_io_tasks, 0)

    def test_submit_cpu_and_pending_count(self):
        """BUG-009: pending_cpu_tasks must accurately count active CPU tasks."""
        started = threading.Event()
        proceed = threading.Event()

        def sync_cpu_task():
            started.set()
            proceed.wait(timeout=2.0)
            return 42 * 42

        future = self.manager.submit_cpu(sync_cpu_task)
        self.assertIsNotNone(future)

        # Wait until task has actually started executing
        started.wait(timeout=2.0)
        self.assertGreaterEqual(self.manager.pending_cpu_tasks, 1)
        self.assertEqual(self.manager.pending_io_tasks, 0)

        # Release task and verify result
        proceed.set()
        result = future.result(timeout=2.0)
        self.assertEqual(result, 1764)

        # Upon completion, count is 0
        self.assertEqual(self.manager.pending_cpu_tasks, 0)

    def test_submit_convenience_helpers(self):
        f_io = submit_io(lambda: "hello_io")
        f_cpu = submit_cpu(lambda: "hello_cpu")

        self.assertEqual(f_io.result(timeout=2.0), "hello_io")
        self.assertEqual(f_cpu.result(timeout=2.0), "hello_cpu")

    def test_shutdown_rejects_new_tasks(self):
        self.manager.shutdown(wait=True)
        self.assertTrue(self.manager.is_shutdown)

        future = self.manager.submit_io(lambda: "late")
        self.assertIsNone(future)


if __name__ == "__main__":
    unittest.main()
