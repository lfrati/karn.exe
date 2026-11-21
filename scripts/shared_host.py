"""Correlated clients for game-serve --host. One process, independent ordered games.

Use SharedHost as a context manager; each session implements play.Engine.ask/send.
Only the host owns process lifetime. Closing a session releases its server-side game.
"""
from concurrent.futures import Future
import json
import subprocess
import threading

from play import Engine, Refused, ROOT
from rust_tools import fast_binary


class SharedHost:
    def __init__(self, executable=None, workers=8):
        self.proc = subprocess.Popen([str(executable or fast_binary('game-serve')), '--host', str(workers)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1, cwd=ROOT)
        ready = json.loads(self.proc.stdout.readline())
        if not ready.get('host'):
            self.proc.kill(); self.proc.wait()
            raise RuntimeError('server does not support shared hosting')
        self._lock = threading.Lock()
        self._write = threading.Lock()
        self._pending = {}
        self._next = 0
        self._game = 0
        self._closed = False
        self._reader = threading.Thread(target=self._read, daemon=True)
        self._reader.start()

    def _read(self):
        failure = RuntimeError('shared engine stopped')
        try:
            for line in self.proc.stdout:
                answer = json.loads(line)
                with self._lock:
                    game, future = self._pending.pop(answer['id'])
                if answer.get('game') != game:
                    future.set_exception(RuntimeError('reply belongs to another game'))
                else:
                    future.set_result(answer)
        except Exception as error:
            failure = error
        finally:
            with self._lock:
                self._closed = True
                pending = list(self._pending.values()); self._pending.clear()
            for _, future in pending:
                future.set_exception(failure)

    def session(self, audit=False):
        with self._lock:
            if self._closed: raise RuntimeError('host closed')
            game = self._game; self._game += 1
        return SharedEngine(self, game, audit)

    def request(self, game, request):
        future = Future()
        with self._write:
            with self._lock:
                if self._closed: raise RuntimeError('host closed')
                identity = self._next; self._next += 1
                self._pending[identity] = (game, future)
            try:
                self.proc.stdin.write(json.dumps({'id':identity, 'game':game, 'request':request})+'\n')
                self.proc.stdin.flush()
            except Exception:
                with self._lock: self._pending.pop(identity, None)
                raise
        return future.result()

    def close(self):
        with self._write:
            with self._lock: self._closed = True
            if not self.proc.stdin.closed: self.proc.stdin.close()
        self.proc.wait(timeout=30)
        self._reader.join(timeout=5)
        self.proc.stdout.close()

    def __enter__(self): return self
    def __exit__(self, *args): self.close()


class SharedEngine(Engine):
    def __init__(self, host, game, audit=False):
        self.host = host; self.game = game; self.proc = host.proc
        self.state = {}; self.audit = audit; self.disclosed = {}
        self._serial = threading.RLock(); self._closed = False

    def send(self, request):
        with self._serial:
            if self._closed: raise RuntimeError('game closed')
            answer = self.host.request(self.game, request)
            if not answer['ok']:
                raise Refused(request, answer)
            return answer['state']

    def ask(self, request):
        with self._serial: return super().ask(request)

    def close(self):
        with self._serial:
            if not self._closed:
                if self.state:
                    self.send({'cmd':'close', 'session_id':self.state['session_id']})
                self._closed = True
