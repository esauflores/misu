#!/usr/bin/env python3
"""VM integration test: portal source selection and a real PipeWire frame.

Run inside the VM session (WM_SOCK/WAYLAND_DISPLAY imported):
    MISU_PORTAL_UI=1 python /opt/misu/test/portal-check.py

The UI mode confirms the picker's first monitor with Enter. Without it, the
chooser FIFO is answered directly. The helper builds against libpipewire and libpng (headers included in their
Arch packages), consumes an actual frame and rejects empty or uniform buffers.
Set MISU_PORTAL_FRAMES=3 with a changing test window to check successive frames.
"""
import os
from pathlib import Path
import subprocess
import time
import tempfile
import shlex
from gi.repository import Gio, GLib
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
responses = {}
def response(conn, sender, path, interface, signal, params):
    responses[path] = params.unpack()
bus.signal_subscribe(None, 'org.freedesktop.portal.Request', 'Response', None, None,
                     Gio.DBusSignalFlags.NONE, response)
context = GLib.MainContext.default()
base = '/org/freedesktop/portal/desktop'
interface = 'org.freedesktop.portal.ScreenCast'
selected = set()
seen = {}
def choose():
    for directory in Path(os.environ['XDG_RUNTIME_DIR']).glob('misu-share.*'):
        options = directory / 'options'
        answer = directory / 'answer'
        if directory in selected or not options.exists() or not answer.exists():
            continue
        lines = options.read_text().splitlines()
        monitors = [line for line in lines if line.startswith('Monitor:')]
        if monitors:
            if os.environ.get('MISU_PORTAL_UI') == '1':
                since = seen.setdefault(directory, time.monotonic())
                if time.monotonic() - since < 4:
                    continue
                subprocess.run(['wm', 'msg', 'seat', '0', 'key', 'Return'], check=True)
                selected.add(directory)
                print('Chooser confirmed with Enter', flush=True)
                continue
            try:
                fd = os.open(answer, os.O_WRONLY | os.O_NONBLOCK)
            except OSError:
                continue
            os.write(fd, (monitors[0] + '\n').encode())
            os.close(fd)
            selected.add(directory)
            print('Chooser selected:', monitors[0], flush=True)
def request(method, signature, args):
    handle, = bus.call_sync('org.freedesktop.portal.Desktop', base, interface, method,
                            GLib.Variant(signature, args), GLib.VariantType.new('(o)'),
                            Gio.DBusCallFlags.NONE, 30000, None).unpack()
    deadline = time.monotonic() + 45
    while handle not in responses and time.monotonic() < deadline:
        while context.pending():
            context.iteration(False)
        choose()
        time.sleep(0.05)
    assert handle in responses, (method, 'response timeout')
    code, result = responses.pop(handle)
    assert code == 0, (method, code, result)
    print(method, result, flush=True)
    return result
session = request('CreateSession', '(a{sv})', ({
    'handle_token': GLib.Variant('s', 'misucreate'),
    'session_handle_token': GLib.Variant('s', 'misusession')},))['session_handle']
request('SelectSources', '(oa{sv})', (session, {
    'handle_token': GLib.Variant('s', 'misuselect'),
    'types': GLib.Variant('u', 1),
    'cursor_mode': GLib.Variant('u', 1),
    'multiple': GLib.Variant('b', False)}))
result = request('Start', '(osa{sv})', (session, '', {'handle_token': GLib.Variant('s', 'misustart')}))
assert result['streams'], result
remote, fds = bus.call_with_unix_fd_list_sync('org.freedesktop.portal.Desktop', base,
    interface, 'OpenPipeWireRemote', GLib.Variant('(oa{sv})', (session, {})),
    GLib.VariantType.new('(h)'), Gio.DBusCallFlags.NONE, 30000, None, None)
fd = fds.get(remote.unpack()[0])
os.fstat(fd)
node = result['streams'][0][0]
print('PipeWire remote opened; stream node', node, flush=True)
with tempfile.TemporaryDirectory(prefix='misu-portal-') as directory:
    binary = str(Path(directory) / 'capture')
    flags = subprocess.check_output(['pkg-config', '--cflags', '--libs', 'libpipewire-0.3', 'libpng'], text=True)
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', str(Path(__file__).with_name('portal-capture.c')),
                    '-o', binary, *shlex.split(flags)], check=True)
    subprocess.run([binary, str(fd), str(node), '/tmp/misu-portal-frame.png',
                    os.environ.get('MISU_PORTAL_FRAMES', '1')], pass_fds=(fd,), check=True, timeout=25)
    print('Portal screen image saved to /tmp/misu-portal-frame.png', flush=True)

os.close(fd)
bus.call_sync('org.freedesktop.portal.Desktop', session, 'org.freedesktop.portal.Session',
              'Close', None, None, Gio.DBusCallFlags.NONE, 3000, None)
