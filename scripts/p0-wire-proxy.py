#!/usr/bin/env python3
"""Proxy TCP ligne : A → 7001 → relais 7000. Journalise les trames complètes.

Instrument de mesure pour les contrôles négatifs P0 (rejeu, clair, usurpation) :
il ne modifie aucun octet, il recopie et journalise les lignes délimitées par
newline telles que vues sur le fil.
"""
import socket
import threading
import time

import os
LOG = open(os.environ.get('P0_WIRE_LOG', '/tmp/p0-wire.log'), 'a', buffering=1)


def pipe(src, dst, tag):
    buf = b''
    try:
        while True:
            d = src.recv(65536)
            if not d:
                break
            dst.sendall(d)
            buf += d
            while b'\n' in buf:
                line, buf = buf.split(b'\n', 1)
                LOG.write(f"{time.time():.3f} {tag} {line.decode('utf-8', 'replace')}\n")
    except OSError:
        pass
    finally:
        try:
            dst.shutdown(socket.SHUT_WR)
        except OSError:
            pass


def handle(c):
    try:
        u = socket.create_connection((os.environ.get('P0_RELAY_HOST', '127.0.0.1'), int(os.environ.get('P0_RELAY_PORT', '7000'))), timeout=10)
    except OSError:
        c.close()
        return
    threading.Thread(target=pipe, args=(c, u, 'A2R'), daemon=True).start()
    pipe(u, c, 'R2A')


s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind((os.environ.get('P0_BIND', '127.0.0.1'), int(os.environ.get('P0_PORT', '7001'))))
s.listen(8)
print('proxy 7001->7000 pret', flush=True)
while True:
    c, _ = s.accept()
    threading.Thread(target=handle, args=(c,), daemon=True).start()
