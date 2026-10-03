#!/usr/bin/env python3
"""Send one agent-selected CLI command to a local owned-task transport."""
import argparse
import json
import socket


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", required=True)
    parser.add_argument("--finish", action="store_true")
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    request = {"finish": True} if args.finish else args.arguments
    payload = json.dumps(request).encode("utf-8")
    if len(payload) > 65_536:
        parser.error("command exceeds 64 KiB")
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(60)
        connection.connect(args.socket)
        connection.sendall(payload)
        connection.shutdown(socket.SHUT_WR)
        output = bytearray()
        while chunk := connection.recv(65_536):
            output.extend(chunk)
            if len(output) > 8 * 1024 * 1024:
                raise ValueError("transport response exceeds 8 MiB")
    if not output:
        raise RuntimeError("transport closed without a result; outcome unknown, do not replay")
    print(output.decode("utf-8"))


if __name__ == "__main__":
    main()
