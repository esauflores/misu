// Run with: node test/network-check.js
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const source = fs.readFileSync(path.join(__dirname, '../modules/bar/plugins/network/Network.qml'), 'utf8');
const root = { _nextId: 1 };
let command;
const context = vm.createContext({ root, requestComp: {
    createObject: (_, request) => { command = request.command; }
}});
for (const name of ['splitTerse', 'bandOf', 'parse', 'connectWifi']) {
    const match = source.match(new RegExp(`    function ${name}\\([^]*?\\n    }`));
    assert.ok(match, name);
    root[name] = vm.runInContext(`(${match[0].trim()})`, context);
}
root.parse(String.raw`##dev
wifi:disconnected
##radio
enabled
##wifi
:Coffee\:Shop:AA\:BB\:CC\:DD\:EE\:FF:80:5180:WPA2
:Unknown:11\:22\:33\:44\:55\:66:50:2412:WPA2
##con
My renamed profile:802-11-wireless:no
Work VPN:vpn:yes
##saved
1234-uuid:Coffee\:Shop
`);
const saved = root.accessPoints[0];
assert.equal(saved.ssid, 'Coffee:Shop');
assert.equal(saved.saved, true);
assert.equal(saved.profileUuid, '1234-uuid');
assert.equal(root.accessPoints[1].saved, false);
assert.equal(root.vpnName, 'Work VPN');
root.connectWifi(saved.ssid, '', saved.bssid, false, saved.profileUuid);
assert.deepEqual(Array.from(command), ['nmcli', 'connection', 'up', 'uuid', '1234-uuid', 'ap', 'AA:BB:CC:DD:EE:FF']);
root.connectWifi('Hidden', 'password', '', true);
assert.deepEqual(Array.from(command), ['nmcli', 'device', 'wifi', 'connect', 'Hidden', 'password', 'password', 'hidden', 'yes']);
console.log('Renamed Wi-Fi profile, escaped SSID, UUID activation and new-network checks passed.');

if (process.argv.includes('--host')) {
    const { execFileSync } = require('node:child_process');
    const snapshot = source.match(/command: \["sh", "-c",([^]*?)\]\n/);
    assert.ok(snapshot);
    context.snapshot = { rescan: false };
    const args = vm.runInContext(`["sh", "-c", ${snapshot[1]}]`, context);
    const data = execFileSync(args[0], Array.from(args).slice(1), { encoding: 'utf8', timeout: 20000 });
    root.parse(data);
    console.log(`Host snapshot parsed: ${root.accessPoints.length} access points, ${root.accessPoints.filter(ap => ap.saved).length} saved matches.`);
}
