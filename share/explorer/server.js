#!/usr/bin/env node

const http = require('http');
const fs = require('fs');
const path = require('path');
const { WebSocketServer } = require('ws');

const RPC_HOST = '127.0.0.1';
const RPC_PORT = 9001;
const WEB_PORT = 8080;

// SECURITY: Rate limiting to prevent DoS attacks
const connectionLimits = new Map();
const MAX_CONNECTIONS_PER_IP = 100; // Increased for development
const RATE_LIMIT_WINDOW = 60000; // 1 minute

function checkRateLimit(ip) {
  const now = Date.now();
  const record = connectionLimits.get(ip) || { count: 0, resetTime: now + RATE_LIMIT_WINDOW };
  
  if (now > record.resetTime) {
    record.count = 0;
    record.resetTime = now + RATE_LIMIT_WINDOW;
  }
  
  if (record.count >= MAX_CONNECTIONS_PER_IP) {
    return false;
  }
  
  record.count++;
  connectionLimits.set(ip, record);
  return true;
}

// State
const state = {
  height: 0,
  miners: new Map(),
  blocks: [],
  transactions: [],
  clients: new Set()
};

// RPC call to Knotcoin node
async function rpc(method, params = []) {
  return new Promise((resolve, reject) => {
    const data = JSON.stringify({
      jsonrpc: '2.0',
      method,
      params,
      id: Date.now()
    });

    // Read auth token from .cookie file
    const fs = require('fs');
    const os = require('os');
    const path = require('path');
    const cookiePath = path.join(os.homedir(), '.knotcoin', 'mainnet', '.cookie');
    let authToken = '';
    try {
      authToken = fs.readFileSync(cookiePath, 'utf8').trim();
    } catch (err) {
      console.error('Failed to read auth token:', err.message);
    }

    // HTTP used for localhost RPC (127.0.0.1:9001) - traffic never leaves machine
    const req = http.request({
      hostname: RPC_HOST,
      port: RPC_PORT,
      path: '/',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Content-Length': data.length,
        'Authorization': `Bearer ${authToken}`
      }
    }, (res) => {
      let body = '';
      res.on('data', chunk => body += chunk);
      res.on('end', () => {
        try {
          const json = JSON.parse(body);
          if (json.error) reject(new Error(json.error.message));
          else resolve(json.result);
        } catch (err) {
          reject(err);
        }
      });
    });

    req.on('error', reject);
    req.write(data);
    req.end();
  });
}

// Broadcast to all WebSocket clients
function broadcast(type, data) {
  const message = JSON.stringify({ type, data });
  state.clients.forEach(client => {
    if (client.readyState === 1) {
      client.send(message);
    }
  });
}

// Poll for new blocks
async function pollBlocks() {
  try {
    const info = await rpc('getmininginfo');
    const newHeight = info.blocks;

    if (newHeight > state.height) {
      // New blocks detected
      for (let h = state.height + 1; h <= newHeight; h++) {
        const hash = await rpc('getblockhash', [h]);
        const block = await rpc('getblock', [hash]);
        
        state.blocks.unshift({ ...block, hash });
        if (state.blocks.length > 50) state.blocks.pop();

        // Track miner
        const miner = block.miner;
        if (!state.miners.has(miner)) {
          state.miners.set(miner, { address: miner, blocks: 0, lastSeen: Date.now() });
        }
        state.miners.get(miner).blocks++;
        state.miners.get(miner).lastSeen = Date.now();

        // Broadcast new block
        broadcast('block', { height: h, hash, miner, time: block.time });
      }

      state.height = newHeight;
      
      // Broadcast updated stats
      broadcast('stats', {
        height: state.height,
        miners: state.miners.size,
        difficulty: info.difficulty,
        hashrate: info.networkhashps || 0
      });
    }
  } catch (err) {
    console.error('Poll error:', err.message);
  }
}

// HTTP server for static files and RPC proxy
const server = http.createServer(async (req, res) => {
  // Add CORS headers
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');
  
  // Add no-cache headers for HTML/JS/CSS to prevent stale data
  if (req.url.endsWith('.html') || req.url.endsWith('.js') || req.url.endsWith('.css') || req.url === '/') {
    res.setHeader('Cache-Control', 'no-cache, no-store, must-revalidate');
    res.setHeader('Pragma', 'no-cache');
    res.setHeader('Expires', '0');
  }
  
  // Handle preflight
  if (req.method === 'OPTIONS') {
    res.writeHead(200);
    res.end();
    return;
  }
  
  // RPC proxy endpoint
  if (req.url === '/rpc' && req.method === 'POST') {
    let body = '';
    req.on('data', chunk => body += chunk);
    req.on('end', async () => {
      try {
        const request = JSON.parse(body);
        const result = await rpc(request.method, request.params || []);
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ jsonrpc: '2.0', result, id: request.id }));
      } catch (err) {
        res.writeHead(500, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ jsonrpc: '2.0', error: { message: err.message }, id: null }));
      }
    });
    return;
  }
  
  const clientIP = req.socket.remoteAddress || req.headers['x-forwarded-for'] || 'unknown';
  
  if (!checkRateLimit(clientIP)) {
    res.writeHead(429, { 
      'Content-Type': 'text/plain',
      'Retry-After': '60'
    });
    res.end('429 Too Many Requests');
    return;
  }

  let filePath = req.url === '/' ? '/index.html' : req.url;
  
  // Remove query string
  filePath = filePath.split('?')[0];
  
  // Security: Normalize and validate path to prevent directory traversal
  const normalizedPath = path.normalize(filePath).replace(/^(\.\.[\/\\])+/, '');
  const safePath = path.join(__dirname, normalizedPath);
  
  // Ensure the resolved path is within the allowed directory
  if (!safePath.startsWith(__dirname)) {
    res.writeHead(403, { 'Content-Type': 'text/plain' });
    res.end('403 Forbidden');
    return;
  }

  const ext = path.extname(safePath);
  const contentTypes = {
    '.html': 'text/html; charset=utf-8',
    '.js': 'text/javascript; charset=utf-8',
    '.css': 'text/css; charset=utf-8',
    '.json': 'application/json',
    '.png': 'image/png',
    '.jpg': 'image/jpg',
    '.svg': 'image/svg+xml',
    '.ico': 'image/x-icon'
  };

  const contentType = contentTypes[ext] || 'text/plain';

  fs.readFile(safePath, (err, content) => {
    if (err) {
      if (err.code === 'ENOENT') {
        res.writeHead(404, { 'Content-Type': 'text/plain' });
        res.end('404 Not Found');
      } else {
        res.writeHead(500, { 'Content-Type': 'text/plain' });
        res.end('500 Internal Server Error');
      }
    } else {
      res.writeHead(200, { 
        'Content-Type': contentType,
        'Cache-Control': 'no-cache'
      });
      res.end(content);
    }
  });
});

// WebSocket server
const wss = new WebSocketServer({ server });

wss.on('connection', (ws) => {
  console.log('Client connected');
  state.clients.add(ws);

  // Send initial state
  ws.send(JSON.stringify({
    type: 'init',
    data: {
      height: state.height,
      miners: Array.from(state.miners.values()),
      blocks: state.blocks.slice(0, 20)
    }
  }));

  ws.on('message', async (message) => {
    try {
      const { method, params, id } = JSON.parse(message);
      
      // Proxy RPC calls
      const result = await rpc(method, params);
      ws.send(JSON.stringify({ id, result }));
    } catch (err) {
      ws.send(JSON.stringify({ id: 0, error: err.message }));
    }
  });

  ws.on('close', () => {
    console.log('Client disconnected');
    state.clients.delete(ws);
  });
});

// Start server
server.listen(WEB_PORT, () => {
  console.log(`
╔════════════════════════════════════════╗
║   KNOTCOIN EXPLORER SERVER             ║
╠════════════════════════════════════════╣
║  Web:       http://localhost:${WEB_PORT}     ║
║  WebSocket: ws://localhost:${WEB_PORT}       ║
║  RPC Node:  ${RPC_HOST}:${RPC_PORT}           ║
╚════════════════════════════════════════╝
  `);

  // Start polling
  setInterval(pollBlocks, 2000); // Poll every 2 seconds
  pollBlocks(); // Initial poll
});
