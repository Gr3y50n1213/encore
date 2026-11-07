"""A TLS server with a throwaway self-signed certificate, for the Android phone run.

    python -I -B tools/android-smoke/tls_server.py CERT.pem KEY.pem URL_FILE

It binds a free port on 127.0.0.1, writes "https://127.0.0.1:<port>" to URL_FILE and
offers the certificate to anyone who connects, until killed. The core on a phone
without EncoreTls.initialize must refuse it with the documented reason (request
core-0001); nothing else is ever sent. Standard library only.
"""

import os
import socket
import ssl
import sys
import threading


def serve_one(ctx, conn):
    try:
        with ctx.wrap_socket(conn, server_side=True) as tls:
            tls.recv(1)
    except (OSError, ssl.SSLError):
        pass  # the client refuses the certificate: that is the expected outcome
    finally:
        conn.close()


def main():
    cert, key, url_file = sys.argv[1:4]
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(cert, key)
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.bind(("127.0.0.1", 0))
    srv.listen(8)
    tmp = url_file + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(f"https://127.0.0.1:{srv.getsockname()[1]}")
    os.replace(tmp, url_file)
    while True:
        conn, _ = srv.accept()
        conn.settimeout(10)
        threading.Thread(target=serve_one, args=(ctx, conn), daemon=True).start()


if __name__ == "__main__":
    main()
