#!/usr/bin/env python3
"""Proxy B → relais (7003 → 7000) avec injection côté B — v2.

Un seul injecteur, visant la connexion B COURANTE (registre global). Les
trames ne sont consommées que si l'envoi réussit ; sinon elles restent dans
inject/ pour la prochaine connexion. Journalise INJECT_OK / INJECT_FAIL.
"""
import glob
import os
import socket
import threading
import time

import os
BASE = os.environ.get('P0_BASE', '/root/p0')
LOG = open(BASE + '/wire-b.log', 'a', buffering=1)
INJ = BASE + '/inject'
DONE = BASE + '/injected'
os.makedirs(INJ, exist_ok=True)
os.makedirs(DONE, exist_ok=True)

CURRENT = {'sock': None, 'lock': threading.Lock()}


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


def injector():
    while True:
        time.sleep(0.2)
        for f in sorted(glob.glob(INJ + '/*.line')):
            data = open(f, 'rb').read()
            with CURRENT['lock']:
                s = CURRENT['sock']
            if s is None:
                continue
            try:
                s.sendall(data if data.endswith(b'\n') else data + b'\n')
                LOG.write(f"{time.time():.3f} INJECT_OK {os.path.basename(f)} {data.decode('utf-8', 'replace')[:120]}\n")
                os.rename(f, DONE + '/' + os.path.basename(f))
            except OSError as e:
                LOG.write(f"{time.time():.3f} INJECT_FAIL {os.path.basename(f)} {e}\n")


def handle(c):
    try:
        u = socket.create_connection((os.environ.get('P0_RELAY_HOST', '127.0.0.1'), int(os.environ.get('P0_RELAY_PORT', '7000'))), timeout=10)
    except OSError:
        c.close()
        return
    with CURRENT['lock']:
        CURRENT['sock'] = c
    LOG.write(f"{time.time():.3f} CONNexion B etablie\n")
    threading.Thread(target=pipe, args=(c, u, 'B2R'), daemon=True).start()
    pipe(u, c, 'R2B')
    with CURRENT['lock']:
        if CURRENT['sock'] is c:
            CURRENT['sock'] = None


s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind((os.environ.get('P0_BIND', '127.0.0.1'), int(os.environ.get('P0_PORT', '7003'))))
s.listen(8)
threading.Thread(target=injector, daemon=True).start()
print('proxy-b v2 7003->7000 pret', flush=True)
while True:
    c, _ = s.accept()
    threading.Thread(target=handle, args=(c,), daemon=True).start()
