#!/usr/bin/env python3
from __future__ import annotations

import argparse
import http.server
import pathlib
import ssl


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--directory", type=pathlib.Path, required=True)
    parser.add_argument("--certificate", type=pathlib.Path, required=True)
    parser.add_argument("--private-key", type=pathlib.Path, required=True)
    parser.add_argument("--port", type=int, default=18443)
    args = parser.parse_args()

    def handler(
        *values: object, **kwargs: object
    ) -> http.server.SimpleHTTPRequestHandler:
        return http.server.SimpleHTTPRequestHandler(
            *values, directory=str(args.directory), **kwargs
        )

    server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(args.certificate, args.private_key)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
