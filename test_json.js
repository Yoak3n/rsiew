const fs = require('fs');
const tauriConf = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json'));
console.log(typeof tauriConf.bundle.resources);
