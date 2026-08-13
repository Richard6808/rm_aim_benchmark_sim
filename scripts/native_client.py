#!/usr/bin/env python3
"""Minimal Native Bridge client for rm_aim_benchmark_sim.

Receives:
  - UDP JSON telemetry on 127.0.0.1:39001
  - TCP JPEG camera stream from 127.0.0.1:39002
Sends:
  - UDP JSON gimbal commands to 127.0.0.1:39000

Install optional preview dependencies:
  pip install numpy opencv-python
"""

from __future__ import annotations

import argparse
import json
import socket
import struct
import threading
import time

MAGIC = b"AIMSIM01"
HEADER = struct.Struct("!8sQQIIII")  # 40 bytes


def recv_exact(sock: socket.socket, n: int) -> bytes:
    data = bytearray()
    while len(data) < n:
        chunk = sock.recv(n - len(data))
        if not chunk:
            raise ConnectionError("camera TCP stream closed")
        data.extend(chunk)
    return bytes(data)


def camera_loop(host: str, port: int, preview: bool) -> None:
    while True:
        try:
            with socket.create_connection((host, port), timeout=3.0) as sock:
                sock.settimeout(None)
                print(f"[camera] connected to {host}:{port}")
                while True:
                    header = recv_exact(sock, HEADER.size)
                    magic, frame_id, timestamp_ns, width, height, jpeg_len, _ = HEADER.unpack(header)
                    if magic != MAGIC:
                        raise RuntimeError(f"bad camera magic: {magic!r}")
                    jpeg = recv_exact(sock, jpeg_len)
                    if preview:
                        import cv2
                        import numpy as np

                        image = cv2.imdecode(np.frombuffer(jpeg, dtype=np.uint8), cv2.IMREAD_COLOR)
                        if image is not None:
                            cv2.putText(
                                image,
                                f"frame {frame_id} {width}x{height}",
                                (20, 35),
                                cv2.FONT_HERSHEY_SIMPLEX,
                                0.8,
                                (255, 255, 255),
                                2,
                            )
                            cv2.imshow("AimSim Native Camera", image)
                            if cv2.waitKey(1) & 0xFF == 27:
                                return
        except Exception as exc:
            print(f"[camera] {exc}; reconnecting...")
            time.sleep(0.5)


def telemetry_loop(bind: str) -> None:
    host, port_text = bind.rsplit(":", 1)
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind((host, int(port_text)))
    print(f"[telemetry] listening on {bind}")
    last_print = 0.0
    while True:
        data, _ = sock.recvfrom(65535)
        packet = json.loads(data)
        now = time.monotonic()
        if now - last_print > 0.25:
            last_print = now
            print(
                "[telemetry] "
                f"yaw={packet['gimbal_yaw_deg']:+.2f} "
                f"pitch={packet['gimbal_pitch_deg']:+.2f} "
                f"HP={packet['target_hp']:.0f}/{packet['target_max_hp']:.0f} "
                f"hit={packet['hit_rate_pct']:.1f}% "
                f"DPS(avg/roll)={packet.get('average_dps', 0.0):.1f}/{packet['rolling_dps']:.1f} "
                f"auto={packet.get('auto_aim_enabled', False)} "
                f"trigger={packet.get('operator_trigger_held', False)} "
                f"advice={packet.get('external_fire_advice', False)}"
            )


def command_loop(target: str) -> None:
    host, port_text = target.rsplit(":", 1)
    addr = (host, int(port_text))
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    print("[command] demo mode: yaw/pitch centered; fire toggles every second. Hold RMB+LMB in AimSim to permit shots.")
    t0 = time.monotonic()
    while True:
        t = time.monotonic() - t0
        packet = {
            "yaw_deg": 0.0,
            "pitch_deg": 0.0,
            "fire": (int(t) % 2) == 0,
        }
        sock.sendto(json.dumps(packet).encode(), addr)
        time.sleep(1.0 / 100.0)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--command", default="127.0.0.1:39000")
    parser.add_argument("--telemetry", default="127.0.0.1:39001")
    parser.add_argument("--camera", default="127.0.0.1:39002")
    parser.add_argument("--preview", action="store_true")
    parser.add_argument("--demo-command", action="store_true")
    args = parser.parse_args()

    camera_host, camera_port = args.camera.rsplit(":", 1)
    threading.Thread(
        target=camera_loop,
        args=(camera_host, int(camera_port), args.preview),
        daemon=True,
    ).start()
    threading.Thread(target=telemetry_loop, args=(args.telemetry,), daemon=True).start()
    if args.demo_command:
        threading.Thread(target=command_loop, args=(args.command,), daemon=True).start()

    while True:
        time.sleep(10.0)


if __name__ == "__main__":
    main()
