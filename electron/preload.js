const { contextBridge } = require('electron');

// Renderer uses this as the RPC base; Electron main runs an authenticated proxy there.
contextBridge.exposeInMainWorld('__KNOTCOIN_RPC__', 'http://localhost:19001');
