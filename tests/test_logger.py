"""Tests for logger utilities (BUG-004: handler leak prevention and reconfiguration)."""

import logging
import os
import tempfile
import unittest
from pathlib import Path

from onyxsh.utils.logger import (
    LogLevel,
    LoggerConfig,
    LoggerManager,
    ThreadSafeLogger,
    get_logger,
    set_console_log_level,
    set_log_to_file_enabled,
)


class TestLoggerManager(unittest.TestCase):
    def setUp(self):
        self.manager = LoggerManager()

    def test_logger_singleton(self):
        m1 = LoggerManager()
        m2 = LoggerManager()
        self.assertIs(m1, m2)

    def test_get_logger_named(self):
        logger = self.manager.get_logger("test.unit.sample")
        self.assertIsInstance(logger, ThreadSafeLogger)
        self.assertEqual(logger.name, "test.unit.sample")

    def test_reconfigure_all_loggers_closes_old_handlers(self):
        """BUG-004: Ensure old handlers are closed on reconfiguration to prevent file descriptor leaks."""
        logger = self.manager.get_logger("test.reconfigure.leak")
        initial_handlers = list(logger._logger.handlers)
        self.assertGreater(len(initial_handlers), 0)

        # Track initial handlers
        file_handlers = [h for h in initial_handlers if isinstance(h, logging.FileHandler)]

        # Trigger reconfigure
        self.manager.reconfigure_all_loggers()

        # The old file handlers must have been closed
        for fh in file_handlers:
            self.assertTrue(fh.stream is None or fh.stream.closed)

    def test_close_logger(self):
        """Ensure closing a ThreadSafeLogger closes and clears its handlers."""
        logger = self.manager.get_logger("test.close.target")
        handlers = list(logger._logger.handlers)
        self.assertGreater(len(handlers), 0)

        logger.close()
        self.assertEqual(len(logger._logger.handlers), 0)

        for h in handlers:
            if isinstance(h, logging.FileHandler):
                self.assertTrue(h.stream is None or h.stream.closed)

    def test_log_methods_do_not_crash(self):
        """Verify standard log methods execute safely."""
        logger = get_logger("test.log.execution")
        logger.debug("Debug message")
        logger.info("Info message")
        logger.warning("Warning message")
        logger.error("Error message")
        logger.critical("Critical message")

    def test_set_console_log_level(self):
        set_console_log_level("DEBUG")
        self.assertEqual(self.manager.config.console_level, LogLevel.DEBUG)
        set_console_log_level("INFO")
        self.assertEqual(self.manager.config.console_level, LogLevel.INFO)


if __name__ == "__main__":
    unittest.main()
