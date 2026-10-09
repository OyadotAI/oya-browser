/** Electron-builder before-pack hook: validate the selected native engine before signing or unpacking. */
module.exports = require('./native-distribution.cjs').beforePack;
