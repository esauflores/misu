#!/usr/bin/env python3
"""Check config generation and seat isolation without a running compositor."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / 'modules/wm/bin'
with tempfile.TemporaryDirectory() as directory:
    temp = Path(directory)
    config = temp / 'config/wm/config.json'
    config.parent.mkdir(parents=True)
    source = (ROOT / 'modules/wm/config.json').read_text()
    config.write_text(source)
    state = temp / 'state/rice'
    env = os.environ | {'XDG_CONFIG_HOME': str(temp / 'config'),
                        'XDG_STATE_HOME': str(temp / 'state')}
    def generate():
        result = subprocess.run([BIN / 'misu-wm-config'], env=env, text=True,
                                capture_output=True, check=True)
        return json.loads(Path(result.stdout.strip()).read_text())
    assert generate() == json.loads(source)
    colors = {'seats': ['#ff0000', '#0000ff'], 'inactive': '#101010'}
    (state / 'wm-colors.json').write_text(json.dumps(colors))
    first = generate()
    assert first['colors'] == colors
    assert generate() == first
    assert config.read_text() == source
    # IPC arguments and spaces must survive the wrappers intact. Seat 0 must
    # be rejected, even if an agent accidentally selects it in the environment.
    fake = temp / 'bin'
    fake.mkdir()
    wm = fake / 'wm'
    wm.write_text('#!/usr/bin/env python3\nimport json, sys\nprint(json.dumps(sys.argv[1:]))\n')
    wm.chmod(0o755)
    env |= {'PATH': str(fake) + ':' + str(BIN) + ':' + env['PATH'], 'MISU_AGENT_SEAT': '1'}
    cases = [(['misu-ai-app', 'open', 'foot', '-e', 'btop'], ['spawn', 'foot', '-e', 'btop']),
             (['misu-ai-pointer', 'type', 'a $value with spaces'], ['type', 'a $value with spaces']),
             (['misu-ai-pointer', 'move', '10', '20'], ['pointer', '10', '20']),
             (['misu-ai-app', 'focus', '42'], ['focus-window', '42']),
             (['misu-ai-shot', '/tmp/test.png'], ['screenshot', '/tmp/test.png']),
             (['misu-ai-workspace', '9'], ['workspace', '9'])]
    for args, expected in cases:
        result = subprocess.run(args, env=env, text=True, capture_output=True, check=True)
        assert json.loads(result.stdout) == ['msg', 'seat', '1'] + expected
    refused = subprocess.run(['misu-agent-seat', 'close'], env=env | {'MISU_AGENT_SEAT': '0'}, capture_output=True)
    assert refused.returncode != 0
    # Screenshots must use the user's screen even while another seat is active.
    tree = {'ok': True, 'result': {'seats': [
        {'id': 0, 'screen': 4, 'active': False}, {'id': 1, 'screen': 7, 'active': True}],
        'screens': [{'id': 4, 'name': 'Virtual-1'}, {'id': 7, 'name': 'Virtual-2'}]}}
    wm.write_text('#!/usr/bin/env python3\nimport json\nprint(' + repr(json.dumps(tree)) + ')\n')
    for name, script in {
        'grim': '#!/usr/bin/env bash\nprintf "%s\\n" "$@" >"$CAPTURE"\ntouch "${@: -1}"\n',
        'wl-copy': '#!/usr/bin/env bash\ncat >/dev/null\n',
        'notify-send': '#!/usr/bin/env bash\nexit 0\n',
    }.items():
        tool = fake / name
        tool.write_text(script)
        tool.chmod(0o755)
    capture = temp / 'grim-args'
    subprocess.run([BIN / 'misu-shot', 'output'], check=True,
                   env=env | {'HOME': str(temp), 'CAPTURE': str(capture)})
    assert capture.read_text().splitlines()[:3] == ['-c', '-o', 'Virtual-1']
    # Palette seeding from a cold installer must not stop a live wallpaper.
    wall = temp / 'wall.webp'
    wall.touch()
    for name, script in {
        'pkill': '#!/usr/bin/env bash\ntouch "$CONTROL"\n',
        'matugen': '#!/usr/bin/env bash\nexit 0\n',
    }.items():
        tool = fake / name
        tool.write_text(script)
        tool.chmod(0o755)
    control = temp / 'wallpaper-stopped'
    offline = env | {'CONTROL': str(control)}
    offline.pop('WAYLAND_DISPLAY', None)
    offline.pop('WM_SOCK', None)
    subprocess.run([BIN / 'misu-wall', 'set', str(wall)], env=offline, check=True)
    assert not control.exists()
print('Palette generation, source preservation, seat isolation, capture and offline wallpaper seeding passed.')
