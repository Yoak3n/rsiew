const fs = require('fs');
const tauriConf = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json'));
console.log(Array.isArray(tauriConf.bundle.resources) ? 'array' : 'map');
