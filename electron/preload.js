const { contextBridge } = require('electron');

// Renderer uses this as the RPC base; Electron main runs an authenticated proxy there.
contextBridge.exposeInMainWorld('__KNOTCOIN_RPC__', 'http://127.0.0.1:19001');
