#!/usr/bin/env python3
"""Generic ROS 2 adapter for the standalone AimSim Native Bridge.

Publishes:
  /aimsim/image/compressed   sensor_msgs/msg/CompressedImage
  /aimsim/camera_info        sensor_msgs/msg/CameraInfo
  /aimsim/gimbal_pose        geometry_msgs/msg/PoseStamped
  /aimsim/muzzle_pose        geometry_msgs/msg/PoseStamped
  /aimsim/camera_pose        geometry_msgs/msg/PoseStamped

Subscribes:
  /aimsim/cmd                std_msgs/msg/Float64MultiArray
                             [yaw_deg, pitch_deg, fire(0/1)]

The adapter intentionally uses only standard ROS 2 messages. Team-specific
messages should be converted here rather than coupled into the simulator core.
"""

from __future__ import annotations

import json
import socket
import struct
import threading
import time

import rclpy
from builtin_interfaces.msg import Time as TimeMsg
from geometry_msgs.msg import PoseStamped
from rclpy.node import Node
from sensor_msgs.msg import CameraInfo, CompressedImage
from std_msgs.msg import Float64MultiArray

MAGIC = b"AIMSIM01"
HEADER = struct.Struct("!8sQQIIII")


def recv_exact(sock, n):
    out = bytearray()
    while len(out) < n:
        chunk = sock.recv(n - len(out))
        if not chunk:
            raise ConnectionError("camera stream closed")
        out.extend(chunk)
    return bytes(out)


def stamp_from_ns(timestamp_ns: int) -> TimeMsg:
    timestamp_ns = int(timestamp_ns)
    msg = TimeMsg()
    msg.sec = timestamp_ns // 1_000_000_000
    msg.nanosec = timestamp_ns % 1_000_000_000
    return msg


class AimSimBridge(Node):
    def __init__(self):
        super().__init__("aimsim_bridge")
        self.command_addr = ("127.0.0.1", 39000)
        self.command_sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.telemetry_sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.telemetry_sock.bind(("127.0.0.1", 39001))

        self.image_pub = self.create_publisher(CompressedImage, "/aimsim/image/compressed", 5)
        self.info_pub = self.create_publisher(CameraInfo, "/aimsim/camera_info", 5)
        self.gimbal_pub = self.create_publisher(PoseStamped, "/aimsim/gimbal_pose", 10)
        self.muzzle_pub = self.create_publisher(PoseStamped, "/aimsim/muzzle_pose", 10)
        self.camera_pub = self.create_publisher(PoseStamped, "/aimsim/camera_pose", 10)
        self.create_subscription(Float64MultiArray, "/aimsim/cmd", self.on_cmd, 10)

        self._camera_info_lock = threading.Lock()
        self._latest_camera_info = None

        threading.Thread(target=self.telemetry_loop, daemon=True).start()
        threading.Thread(target=self.camera_loop, daemon=True).start()

    def on_cmd(self, msg: Float64MultiArray):
        if len(msg.data) < 2:
            return
        packet = {
            "yaw_deg": float(msg.data[0]),
            "pitch_deg": float(msg.data[1]),
            "fire": bool(msg.data[2]) if len(msg.data) >= 3 else False,
        }
        self.command_sock.sendto(json.dumps(packet).encode(), self.command_addr)

    @staticmethod
    def pose_msg(packet, key):
        p = packet[key]
        msg = PoseStamped()
        msg.header.stamp = stamp_from_ns(packet["timestamp_ns"])
        msg.header.frame_id = "world"
        msg.pose.position.x, msg.pose.position.y, msg.pose.position.z = p["translation_m"]
        q = p["quaternion_xyzw"]
        msg.pose.orientation.x = q[0]
        msg.pose.orientation.y = q[1]
        msg.pose.orientation.z = q[2]
        msg.pose.orientation.w = q[3]
        return msg

    @staticmethod
    def camera_info_msg(ci, timestamp_ns):
        info = CameraInfo()
        info.header.stamp = stamp_from_ns(timestamp_ns)
        info.header.frame_id = "camera"
        info.width = ci["width"]
        info.height = ci["height"]
        info.k = [ci["fx"], 0.0, ci["cx"], 0.0, ci["fy"], ci["cy"], 0.0, 0.0, 1.0]
        info.p = [ci["fx"], 0.0, ci["cx"], 0.0, 0.0, ci["fy"], ci["cy"], 0.0, 0.0, 0.0, 1.0, 0.0]
        return info

    def telemetry_loop(self):
        while rclpy.ok():
            data, _ = self.telemetry_sock.recvfrom(65535)
            packet = json.loads(data)
            self.gimbal_pub.publish(self.pose_msg(packet, "gimbal_pose"))
            self.muzzle_pub.publish(self.pose_msg(packet, "muzzle_pose"))
            self.camera_pub.publish(self.pose_msg(packet, "camera_pose"))
            with self._camera_info_lock:
                self._latest_camera_info = packet["camera_info"].copy()

    def camera_loop(self):
        while rclpy.ok():
            try:
                with socket.create_connection(("127.0.0.1", 39002), timeout=3.0) as sock:
                    sock.settimeout(None)
                    while rclpy.ok():
                        header = recv_exact(sock, HEADER.size)
                        magic, frame_id, timestamp_ns, width, height, jpeg_len, _ = HEADER.unpack(header)
                        if magic != MAGIC:
                            raise RuntimeError("camera protocol desync")
                        jpeg = recv_exact(sock, jpeg_len)

                        stamp = stamp_from_ns(timestamp_ns)
                        msg = CompressedImage()
                        msg.header.stamp = stamp
                        msg.header.frame_id = "camera"
                        msg.format = "jpeg"
                        msg.data = jpeg
                        self.image_pub.publish(msg)

                        with self._camera_info_lock:
                            ci = None if self._latest_camera_info is None else self._latest_camera_info.copy()
                        if ci is not None:
                            # Keep image and CameraInfo on the same simulator-produced timestamp.
                            ci["width"] = width
                            ci["height"] = height
                            self.info_pub.publish(self.camera_info_msg(ci, timestamp_ns))
            except Exception as exc:
                self.get_logger().warning(f"camera reconnect: {exc}")
                time.sleep(0.5)


def main():
    rclpy.init()
    node = AimSimBridge()
    try:
        rclpy.spin(node)
    finally:
        node.destroy_node()
        rclpy.shutdown()


if __name__ == "__main__":
    main()
