import os
import socket
import struct
import threading
from pathlib import Path
from typing import Callable, Optional, Tuple

from ..utils.logger import get_logger


class TftpServerError(Exception):
    """Base exception for TFTP server failures."""


class TftpBindError(TftpServerError):
    """Raised when the TFTP server cannot bind its UDP socket."""


class TftpFileError(TftpServerError):
    """Raised when the TFTP server cannot access its configured directories."""


class TftpNetworkError(TftpServerError):
    """Raised when the TFTP server hits an unexpected network failure."""


class TftpServer:
    """Small RFC 1350-compatible TFTP server for RRQ/WRQ octet transfers."""

    OP_RRQ = 1
    OP_WRQ = 2
    OP_DATA = 3
    OP_ACK = 4
    OP_ERROR = 5

    ERR_UNDEFINED = 0
    ERR_NOT_FOUND = 1
    ERR_ACCESS = 2
    ERR_DISK_FULL = 3
    ERR_ILLEGAL_OP = 4
    ERR_UNKNOWN_ID = 5
    ERR_EXISTS = 6

    BLOCK_SIZE = 512
    DEFAULT_TIMEOUT = 2.0
    DEFAULT_RETRIES = 3

    _ERROR_MESSAGES = {
        ERR_UNDEFINED: "Undefined error",
        ERR_NOT_FOUND: "File not found",
        ERR_ACCESS: "Access violation",
        ERR_DISK_FULL: "Disk full or allocation exceeded",
        ERR_ILLEGAL_OP: "Illegal TFTP operation",
        ERR_UNKNOWN_ID: "Unknown transfer ID",
        ERR_EXISTS: "File already exists",
    }

    def __init__(
        self,
        on_running_changed: Optional[Callable[[bool], None]] = None,
        on_error: Optional[Callable[[Exception], None]] = None,
    ):
        self.logger = get_logger("onyxsh.filemanager.tftp_server")
        self._on_running_changed = on_running_changed
        self._on_error = on_error
        self._stop_event = threading.Event()
        self._thread: Optional[threading.Thread] = None
        self._socket: Optional[socket.socket] = None
        self._lock = threading.RLock()
        self._running = False
        self.port = 69
        self.upload_dir = Path.home()
        self.download_dir = Path.home()

    @property
    def is_running(self) -> bool:
        with self._lock:
            return self._running

    def start(self, port: int, upload_dir: str, download_dir: str) -> None:
        if self.is_running:
            self.stop()

        upload_path = Path(upload_dir).expanduser().resolve()
        download_path = Path(download_dir).expanduser().resolve()
        if not upload_path.is_dir() or not os.access(upload_path, os.R_OK):
            raise TftpFileError(f"Upload directory is not readable: {upload_path}")
        if not download_path.is_dir() or not os.access(download_path, os.W_OK):
            raise TftpFileError(f"Download directory is not writable: {download_path}")
        if port < 0 or port > 65535:
            raise TftpBindError(f"Invalid UDP port: {port}")

        with self._lock:
            self.port = port
            self.upload_dir = upload_path
            self.download_dir = download_path
            self._stop_event.clear()
            self._thread = threading.Thread(
                target=self._serve, name="ZashTerminalTftpServer", daemon=True
            )
            self._thread.start()

    def stop(self) -> None:
        thread = None
        with self._lock:
            self._stop_event.set()
            thread = self._thread
            if self._socket is not None:
                try:
                    self._socket.close()
                except OSError:
                    pass
        if thread and thread.is_alive() and thread is not threading.current_thread():
            thread.join(timeout=2.5)
        with self._lock:
            self._thread = None

    def _set_running(self, running: bool) -> None:
        with self._lock:
            if self._running == running:
                return
            self._running = running
        if self._on_running_changed:
            self._on_running_changed(running)

    def _emit_error(self, error: Exception) -> None:
        self.logger.error(f"TFTP server error: {error}")
        if self._on_error:
            self._on_error(error)

    def _serve(self) -> None:
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.settimeout(0.5)
        try:
            sock.bind(("", self.port))
        except OSError as exc:
            sock.close()
            self._set_running(False)
            self._emit_error(TftpBindError(str(exc)))
            return

        with self._lock:
            self._socket = sock
        self._set_running(True)

        try:
            while not self._stop_event.is_set():
                try:
                    data, client = sock.recvfrom(2048)
                except socket.timeout:
                    continue
                except OSError:
                    if self._stop_event.is_set():
                        break
                    raise

                threading.Thread(
                    target=self._handle_request,
                    args=(data, client),
                    name=f"ZashTerminalTftpTransfer-{client[0]}:{client[1]}",
                    daemon=True,
                ).start()
        except Exception as exc:
            if not self._stop_event.is_set():
                self._emit_error(TftpNetworkError(str(exc)))
        finally:
            try:
                sock.close()
            except OSError:
                pass
            with self._lock:
                if self._socket is sock:
                    self._socket = None
            self._set_running(False)

    def _handle_request(self, data: bytes, client: Tuple[str, int]) -> None:
        transfer_sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        transfer_sock.settimeout(self.DEFAULT_TIMEOUT)
        try:
            try:
                opcode, filename, mode = self._parse_request(data)
            except ValueError:
                self._send_error(transfer_sock, client, self.ERR_ILLEGAL_OP)
                return

            if mode not in ("octet", "netascii"):
                self._send_error(transfer_sock, client, self.ERR_ILLEGAL_OP)
                return

            if opcode == self.OP_RRQ:
                self._serve_read_request(transfer_sock, client, filename)
            elif opcode == self.OP_WRQ:
                self._serve_write_request(transfer_sock, client, filename)
            else:
                self._send_error(transfer_sock, client, self.ERR_ILLEGAL_OP)
        except OSError as exc:
            self.logger.warning(f"TFTP transfer failed for {client}: {exc}")
        finally:
            transfer_sock.close()

    def _parse_request(self, data: bytes) -> Tuple[int, str, str]:
        if len(data) < 4:
            raise ValueError("Packet too short")
        opcode = struct.unpack("!H", data[:2])[0]
        parts = data[2:].split(b"\0")
        if len(parts) < 3 or not parts[0] or not parts[1]:
            raise ValueError("Malformed request")
        filename = parts[0].decode("utf-8", errors="strict")
        mode = parts[1].decode("ascii", errors="strict").lower()
        return opcode, filename, mode

    def _resolve_child(self, root: Path, requested: str) -> Optional[Path]:
        requested = requested.replace("\\", "/").lstrip("/")
        if not requested or "\0" in requested:
            return None
        candidate = (root / requested).resolve()
        try:
            candidate.relative_to(root)
        except ValueError:
            return None
        return candidate

    def _serve_read_request(
        self, sock: socket.socket, client: Tuple[str, int], filename: str
    ) -> None:
        path = self._resolve_child(self.upload_dir, filename)
        if path is None:
            self._send_error(sock, client, self.ERR_ACCESS)
            return
        if not path.is_file():
            self._send_error(sock, client, self.ERR_NOT_FOUND)
            return
        if not os.access(path, os.R_OK):
            self._send_error(sock, client, self.ERR_ACCESS)
            return

        block = 1
        try:
            with path.open("rb") as file:
                while not self._stop_event.is_set():
                    payload = file.read(self.BLOCK_SIZE)
                    packet = struct.pack("!HH", self.OP_DATA, block) + payload
                    if not self._send_with_ack(sock, client, packet, block):
                        return
                    block = (block + 1) & 0xFFFF
                    if len(payload) < self.BLOCK_SIZE:
                        return
        except OSError:
            self._send_error(sock, client, self.ERR_ACCESS)

    def _serve_write_request(
        self, sock: socket.socket, client: Tuple[str, int], filename: str
    ) -> None:
        path = self._resolve_child(self.download_dir, filename)
        if path is None:
            self._send_error(sock, client, self.ERR_ACCESS)
            return
        if path.exists():
            self._send_error(sock, client, self.ERR_EXISTS)
            return

        try:
            path.parent.mkdir(parents=True, exist_ok=True)
            with path.open("xb") as file:
                self._send_ack(sock, client, 0)
                expected_block = 1
                while not self._stop_event.is_set():
                    data, sender = sock.recvfrom(self.BLOCK_SIZE + 4)
                    if sender != client:
                        self._send_error(sock, sender, self.ERR_UNKNOWN_ID)
                        continue
                    if len(data) < 4:
                        self._send_error(sock, client, self.ERR_ILLEGAL_OP)
                        return
                    opcode, block = struct.unpack("!HH", data[:4])
                    if opcode != self.OP_DATA:
                        self._send_error(sock, client, self.ERR_ILLEGAL_OP)
                        return
                    if block != expected_block:
                        self._send_ack(sock, client, (expected_block - 1) & 0xFFFF)
                        continue
                    payload = data[4:]
                    file.write(payload)
                    self._send_ack(sock, client, block)
                    expected_block = (expected_block + 1) & 0xFFFF
                    if len(payload) < self.BLOCK_SIZE:
                        return
        except FileExistsError:
            self._send_error(sock, client, self.ERR_EXISTS)
        except OSError:
            self._send_error(sock, client, self.ERR_ACCESS)

    def _send_with_ack(
        self, sock: socket.socket, client: Tuple[str, int], packet: bytes, block: int
    ) -> bool:
        for _attempt in range(self.DEFAULT_RETRIES):
            sock.sendto(packet, client)
            try:
                while not self._stop_event.is_set():
                    data, sender = sock.recvfrom(4)
                    if sender != client:
                        self._send_error(sock, sender, self.ERR_UNKNOWN_ID)
                        continue
                    if len(data) < 4:
                        continue
                    opcode, ack_block = struct.unpack("!HH", data[:4])
                    if opcode == self.OP_ACK and ack_block == block:
                        return True
                    if opcode == self.OP_ERROR:
                        return False
            except socket.timeout:
                continue
        return False

    def _send_ack(self, sock: socket.socket, client: Tuple[str, int], block: int) -> None:
        sock.sendto(struct.pack("!HH", self.OP_ACK, block), client)

    def _send_error(
        self, sock: socket.socket, client: Tuple[str, int], code: int
    ) -> None:
        message = self._ERROR_MESSAGES.get(
            code, self._ERROR_MESSAGES[self.ERR_UNDEFINED]
        )
        packet = (
            struct.pack("!HH", self.OP_ERROR, code) + message.encode("ascii") + b"\0"
        )
        sock.sendto(packet, client)
