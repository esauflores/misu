#!/usr/bin/env python3
"""Run without a desktop or root: python3 test/simplification-check.py."""
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
helper = (ROOT / 'modules/apps/zen/zen-policies').read_text()
with tempfile.TemporaryDirectory() as directory:
    temp = Path(directory)
    zen = temp / 'installed.json'
    ours = {'policies': {'PasswordManagerEnabled': False, 'ExtensionSettings': {
        'misu': {'installation_mode': 'force_installed', 'private_browsing': True}}}}
    original = {'policies': {'Unrelated': True, 'ExtensionSettings': {
        'personal': {'installation_mode': 'allowed'},
        'misu': {'custom_setting': 'keep'}}}}
    (temp / 'policies.json').write_text(json.dumps(ours))
    zen.write_text(json.dumps(original))
    script = temp / 'zen-policies'
    script.write_text(helper.replace('zen=/opt/zen-browser-bin/distribution/policies.json',
                                     f'zen={zen}'))
    def run(action):
        subprocess.run(['bash', str(script), action], check=True)
        return json.loads(zen.read_text())
    applied = run('apply')
    assert run('apply') == applied
    # A user change made after install must survive removal.
    applied['policies']['ExtensionSettings']['misu']['private_browsing'] = False
    zen.write_text(json.dumps(applied))
    removed = run('remove')
    expected = json.loads(json.dumps(original))
    expected['policies']['ExtensionSettings']['misu']['private_browsing'] = False
    assert removed == expected, removed
    assert run('remove') == expected
print('Nested policy preservation passed.')
