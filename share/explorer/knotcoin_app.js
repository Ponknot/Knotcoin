/* Knotcoin v1.0.2 — Desktop Wallet Application
   Connects to knotcoind via JSON-RPC on 127.0.0.1:9001
   Authenticates using Bearer token from ~/.knotcoin/mainnet/.cookie */

(function () {
  'use strict';

  var RPC_URL = 'http://127.0.0.1:9001';
  var POLL_MS = 2500;
  var KNOTS_PER_KOT = 1e8;

  var state = {
    mnemonic: null,
    address: null,
    balance: 0,
    height: 0,
    difficulty: '',
    miners: [],
    isMining: false,
    miningThreads: 2,
    miningStart: 0,
    miningBlocksFound: 0,
    connected: false,
    authToken: '',
    pollTimer: null,
    connectRetries: 0,
    explorerPage: 1,
    txHistory: [],
    lastTxHistoryHeight: -1
  };

  /* RPC CLIENT */

  function rpc(method, params) {
    var body = JSON.stringify({
      jsonrpc: '2.0',
      id: Date.now(),
      method: method,
      params: params || []
    });
    var headers = { 'Content-Type': 'application/json' };
    if (state.authToken) {
      headers['Authorization'] = 'Bearer ' + state.authToken;
    }
    return fetch(RPC_URL, { method: 'POST', headers: headers, body: body })
      .then(function (r) { return r.json(); })
      .then(function (j) {
        if (j.error) throw new Error(j.error.message || JSON.stringify(j.error));
        return j.result;
      });
  }

  /* HELPERS */

  function el(id) { return document.getElementById(id); }
  function knots2kot(knots) { return (knots / KNOTS_PER_KOT).toFixed(8); }
  function shortAddr(addr) {
    if (!addr || addr.length < 16) return addr || '';
    return addr.slice(0, 10) + '...' + addr.slice(-6);
  }

  function toast(msg, type) {
    var t = document.createElement('div');
    t.className = 'toast ' + (type || 'success');
    t.textContent = msg;
    el('toasts').appendChild(t);
    setTimeout(function () { t.remove(); }, 4000);
  }

  var MAX_TARGET_HEX = '7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff';

  function hexToFloat(hex) {
    if (!hex || hex.length < 16) return 0;
    var top = parseInt(hex.slice(0, 8), 16);
    var bot = parseInt(hex.slice(8, 16), 16);
    return top * 4294967296 + bot;
  }

  function formatDifficulty(hex) {
    if (!hex) return '1.00';
    var maxVal = hexToFloat(MAX_TARGET_HEX);
    var curVal = hexToFloat(hex);
    if (curVal <= 0) return '1.00';
    var diff = maxVal / curVal;
    if (diff >= 1e12) return (diff / 1e12).toFixed(2) + ' T';
    if (diff >= 1e9) return (diff / 1e9).toFixed(2) + ' G';
    if (diff >= 1e6) return (diff / 1e6).toFixed(2) + ' M';
    if (diff >= 1e3) return (diff / 1e3).toFixed(2) + ' K';
    return diff.toFixed(2);
  }

  function formatHashrate(rate) {
    if (!rate || rate <= 0) return '0 H/s';
    if (rate >= 1e15) return (rate / 1e15).toFixed(2) + ' PH/s';
    if (rate >= 1e12) return (rate / 1e12).toFixed(2) + ' TH/s';
    if (rate >= 1e9) return (rate / 1e9).toFixed(2) + ' GH/s';
    if (rate >= 1e6) return (rate / 1e6).toFixed(2) + ' MH/s';
    if (rate >= 1e3) return (rate / 1e3).toFixed(2) + ' KH/s';
    return rate.toFixed(2) + ' H/s';
  }

  function estimateNetworkHashrate(diffHex, blockTimeSecs) {
    if (!diffHex) return 0;
    var maxVal = hexToFloat(MAX_TARGET_HEX);
    var curVal = hexToFloat(diffHex);
    if (curVal <= 0) return 0;
    var diff = maxVal / curVal;
    var effectiveTime = Math.max(30, blockTimeSecs || 60);
    return (diff * 4294967296) / effectiveTime;
  }

  function timeAgo(ts) {
    var diff = Math.floor(Date.now() / 1000) - ts;
    if (diff < 0) return 'just now';
    if (diff < 60) return diff + 's ago';
    if (diff < 3600) return Math.floor(diff / 60) + 'm ago';
    if (diff < 86400) return Math.floor(diff / 3600) + 'h ago';
    return Math.floor(diff / 86400) + 'd ago';
  }

  function formatUptime(seconds) {
    if (!seconds || seconds <= 0) return '0s';
    var h = Math.floor(seconds / 3600);
    var m = Math.floor((seconds % 3600) / 60);
    var s = seconds % 60;
    if (h > 0) return h + 'h ' + m + 'm ' + s + 's';
    if (m > 0) return m + 'm ' + s + 's';
    return s + 's';
  }

  /* AUTH */

  function loadAuth() {
    return new Promise(function (resolve) {
      if (window.__TAURI__) {
        try {
          var invoke = window.__TAURI__.invoke || (window.__TAURI__.core && window.__TAURI__.core.invoke);
          if (invoke) {
            invoke('get_rpc_auth_token')
              .then(function (token) { state.authToken = token; resolve(); })
              .catch(function () { setTimeout(function () { retryAuth(resolve, 0); }, 1500); });
          } else { fallbackAuth(resolve); }
        } catch (e) { fallbackAuth(resolve); }
      } else { fallbackAuth(resolve); }
    });
  }

  function fallbackAuth(resolve) {
    var token = localStorage.getItem('knotcoin_auth') || prompt('Enter RPC Auth Token (from ~/.knotcoin/mainnet/.cookie):');
    if (token) { state.authToken = token; localStorage.setItem('knotcoin_auth', token); }
    resolve();
  }

  function retryAuth(resolve, attempt) {
    if (attempt > 10) return fallbackAuth(resolve);
    var invoke = window.__TAURI__ && (window.__TAURI__.invoke || (window.__TAURI__.core && window.__TAURI__.core.invoke));
    if (invoke) {
      invoke('get_rpc_auth_token')
        .then(function (token) { state.authToken = token; resolve(); })
        .catch(function () { setTimeout(function () { retryAuth(resolve, attempt + 1); }, 1000); });
    } else { fallbackAuth(resolve); }
  }

  /* WAIT FOR NODE */

  function waitForNode() {
    return new Promise(function (resolve) { tryConnect(resolve, 0); });
  }

  function tryConnect(resolve, attempt) {
    rpc('getblockcount')
      .then(function () {
        state.connected = true;
        el('node-dot').className = 'status-dot connected';
        el('node-label').textContent = 'Connected';
        resolve();
      })
      .catch(function () {
        state.connectRetries = attempt;
        el('node-dot').className = 'status-dot syncing';
        el('node-label').textContent = 'Connecting... (' + attempt + ')';
        if (attempt < 30) {
          setTimeout(function () { tryConnect(resolve, attempt + 1); }, 1000);
        } else {
          el('node-dot').className = 'status-dot';
          el('node-label').textContent = 'Node offline';
          resolve();
        }
      });
  }

  /* ONBOARDING */

  window.showOnboard = function (panel) {
    ['welcome', 'referral', 'create', 'import'].forEach(function (p) {
      var e = el('onboard-' + p);
      if (e) e.classList.toggle('hidden', p !== panel);
    });
    if (panel === 'create') generateMnemonic();
  };

  function generateMnemonic() {
    el('mnemonic-words').textContent = 'Generating...';
    el('btn-confirm-create').disabled = true;
    rpc('wallet_create')
      .then(function (r) {
        state.mnemonic = r.mnemonic;
        state.address = r.address;
        el('mnemonic-words').textContent = r.mnemonic;
        el('btn-confirm-create').disabled = false;
      })
      .catch(function (err) {
        el('mnemonic-words').textContent = 'Error: ' + err.message;
        el('btn-confirm-create').disabled = true;
      });
  }

  window.confirmCreateWallet = function () {
    if (!state.mnemonic) return;
    var referral = el('create-referral').value.trim();
    if (referral.startsWith('knotcoin:?ref=')) referral = referral.replace('knotcoin:?ref=', '').trim();
    localStorage.setItem('knotcoin_mnemonic', state.mnemonic);
    localStorage.setItem('knotcoin_address', state.address);
    if (referral) localStorage.setItem('knotcoin_referrer', referral);

    if (referral) {
      toast('Registering referral on-chain...', 'info');
      rpc('wallet_register_referral', [state.mnemonic, referral])
        .then(function () { toast('Wallet created & Referral bound!', 'success'); enterApp(); })
        .catch(function () {
          toast('Wallet created. Referral will bind on first mine or deposit.', 'info');
          enterApp();
        });
    } else {
      toast('Wallet created: ' + shortAddr(state.address), 'success');
      enterApp();
    }
  };

  window.confirmImportWallet = function () {
    var mnemonic = el('import-mnemonic').value.trim();
    if (!mnemonic) { toast('Enter a valid mnemonic', 'error'); return; }
    state.mnemonic = mnemonic;
    localStorage.setItem('knotcoin_mnemonic', mnemonic);
    rpc('wallet_get_address', [mnemonic])
      .then(function (r) {
        state.address = r.address;
        localStorage.setItem('knotcoin_address', r.address);
        toast('Wallet imported: ' + shortAddr(r.address), 'success');
        enterApp();
      })
      .catch(function (err) { toast('Import failed: ' + err.message, 'error'); });
  };

  function enterApp() {
    el('onboarding').classList.add('hidden');
    el('app').classList.remove('hidden');
    startPolling();
  }

  /* NAV */

  function initNav() {
    document.querySelectorAll('.nav-item').forEach(function (item) {
      item.addEventListener('click', function () {
        document.querySelectorAll('.nav-item').forEach(function (i) { i.classList.remove('active'); });
        item.classList.add('active');
        var tab = item.getAttribute('data-tab');
        document.querySelectorAll('.tab-content').forEach(function (tc) {
          tc.classList.toggle('active', tc.id === 'tab-' + tab);
        });
        if (tab === 'explorer') updateExplorerBlocks();
        if (tab === 'wallet') updateTxHistory();
      });
    });
  }

  /* POLLING */

  function startPolling() {
    pollData();
    state.pollTimer = setInterval(pollData, POLL_MS);
    setInterval(updateMiningTick, 1000);
  }

  function pollData() {
    console.log('🔄 Syncing data... (Block ' + state.height + ')');
    pollMiningInfo();
    pollPeerInfo();
    pollBalance();
    pollMiners();
    pollMiningStatus();
    updateReferrals();
    updateGovernance();
  }

  function pollMiningInfo() {
    var startTime = performance.now();
    rpc('getmininginfo')
      .then(function (info) {
        state.connected = true;
        state.height = info.blocks || 0;
        state.difficulty = info.difficulty || '';
        el('node-dot').className = 'status-dot connected';

        var ping = Math.round(performance.now() - startTime);
        var now = new Date();
        var timeStr = now.getHours().toString().padStart(2, '0') + ':' + 
                      now.getMinutes().toString().padStart(2, '0') + ':' + 
                      now.getSeconds().toString().padStart(2, '0');
        el('node-label').textContent = 'Block ' + state.height + ' • ' + timeStr + ' (' + ping + 'ms)';
        el('stat-height').textContent = state.height.toLocaleString();
        el('stat-difficulty').textContent = formatDifficulty(info.difficulty);
        el('stat-mempool').textContent = (info.mempool || 0) + ' tx';
        el('settings-height').textContent = state.height.toLocaleString();

        computeNetworkHashrate();
      })
      .catch(function () {
        state.connected = false;
        el('node-dot').className = 'status-dot';
        el('node-label').textContent = 'Disconnected';
      });
  }

  function computeNetworkHashrate() {
    if (state.height <= 0) { el('stat-hashrate').textContent = '0 H/s'; return; }
    rpc('getblockhash', [state.height])
      .then(function (hash) { return rpc('getblock', [hash]); })
      .then(function (b) {
        var elapsed = Math.floor(Date.now() / 1000) - b.time;
        if (elapsed > 600) {
          el('stat-hashrate').textContent = '0 H/s';
        } else {
          var rate = estimateNetworkHashrate(state.difficulty, Math.max(60, elapsed));
          el('stat-hashrate').textContent = formatHashrate(rate);
        }
      })
      .catch(function () { el('stat-hashrate').textContent = '0 H/s'; });
  }

  function pollPeerInfo() {
    rpc('getpeerinfo')
      .then(function (peers) {
        var count = peers.peer_count || 0;
        el('stat-peers').textContent = count;
        el('settings-peers').textContent = count;
      })
      .catch(function () {
        if (el('stat-peers')) el('stat-peers').textContent = '0';
      });
  }

  function pollBalance() {
    if (!state.address) return;
    rpc('getbalance', [state.address])
      .then(function (b) {
        state.balance = b.balance_knots || 0;
        el('wallet-balance').innerHTML = knots2kot(state.balance) + '<span class="balance-unit">KOT</span>';
        el('wallet-address').textContent = state.address;
        el('receive-address').textContent = state.address;
        el('mining-address').textContent = shortAddr(state.address);
      })
      .catch(function () { });
  }

  function pollMiners() {
    rpc('get_all_miners')
      .then(function (data) {
        if (data && data.miners) {
          state.miners = data.miners;
          el('stat-holders').textContent = data.miners.length;
          updateNetworkViz(data.miners);
          updateRecentBlocks();
        }
      })
      .catch(function () { });
  }

  function pollMiningStatus() {
    if (!state.connected) return;
    rpc('get_mining_status')
      .then(function (res) {
        state.isMining = res.active;
        state.miningBlocksFound = res.blocks_found || 0;
        if (res.active && res.uptime_seconds > 0) {
          state.miningStart = Date.now() - (res.uptime_seconds * 1000);
        }
        updateMiningUI();
      })
      .catch(function () { });
  }

  /* RECENT BLOCKS */

  function updateRecentBlocks() {
    if (state.height < 0) return;
    var tbody = el('recent-blocks');
    var start = Math.max(0, state.height - 4);
    var promises = [];
    for (var h = state.height; h >= start; h--) {
      promises.push(
        rpc('getblockhash', [h]).then(function (hash) { return rpc('getblock', [hash]); }).catch(function () { return null; })
      );
    }
    Promise.all(promises).then(function (blocks) {
      var valid = blocks.filter(function (b) { return b !== null; });
      valid.sort(function (a, b) { return b.height - a.height; });
      var html = '';
      valid.forEach(function (b) {
        var reward = calculateReward(b.height);
        html += '<tr class="clickable-block" data-height="' + b.height + '" style="cursor:pointer">' +
          '<td class="mono">' + b.height + '</td>' +
          '<td><span class="addr-short" title="' + b.miner + '">' + shortAddr(b.miner) + '</span></td>' +
          '<td class="mono">' + (b.tx_count || 0) + '</td>' +
          '<td class="mono">' + knots2kot(reward) + ' KOT</td>' +
          '<td>' + timeAgo(b.time) + '</td></tr>';
      });
      if (html) tbody.innerHTML = html;
    });
  }

  function calculateReward(height) {
    if (height <= 262800) return Math.round((0.1 + 0.9 * height / 262800) * KNOTS_PER_KOT);
    if (height <= 525600) return KNOTS_PER_KOT;
    var adjusted = height - 525601;
    var x = adjusted + 2;
    if (x <= 2) return KNOTS_PER_KOT;
    return Math.round(KNOTS_PER_KOT / Math.log2(x));
  }

  /* TRANSACTION HISTORY */

  function updateTxHistory() {
    if (!state.address || state.height === state.lastTxHistoryHeight) return;
    state.lastTxHistoryHeight = state.height;

    rpc('gettransactionhistory', [state.address, 50])
      .then(function (data) {
        state.txHistory = data.transactions || [];
        renderTxHistory();
      })
      .catch(function () { });
  }

  function renderTxHistory() {
    var tbody = el('tx-history');
    if (!state.txHistory.length) {
      tbody.innerHTML = '<tr><td colspan="5" style="text-align:center;color:var(--text-muted)">No transactions yet</td></tr>';
      return;
    }
    var html = '';
    state.txHistory.forEach(function (tx) {
      var typeClass = tx.type === 'sent' ? 'red' : tx.type === 'received' ? 'blue' : 'green';
      var typeLabel = tx.type === 'mining_reward' ? 'Mined' : tx.type === 'sent' ? 'Sent' : 'Received';
      var sign = tx.type === 'sent' ? '-' : '+';
      html += '<tr>' +
        '<td><span class="badge badge-' + typeClass + '">' + typeLabel + '</span></td>' +
        '<td><span class="addr-short" title="' + tx.address + '">' + shortAddr(tx.address) + '</span></td>' +
        '<td class="mono">' + sign + tx.amount_kot + ' KOT</td>' +
        '<td class="mono">' + knots2kot(tx.fee_knots || 0) + '</td>' +
        '<td class="mono">' + tx.block_height + '</td>' +
        '</tr>';
    });
    tbody.innerHTML = html;
  }

  /* EXPLORER */

  function updateExplorerBlocks() {
    if (state.height < 0) return;
    var tbody = el('explorer-blocks');
    var limit = 15;
    var start = state.height - ((state.explorerPage - 1) * limit);
    var end = Math.max(0, start - limit + 1);
    if (start < 0) return;

    el('explorer-page-info').textContent = 'Page ' + state.explorerPage;
    el('btn-explorer-prev').disabled = (state.explorerPage <= 1);
    el('btn-explorer-next').disabled = (end <= 0);

    var promises = [];
    for (var h = start; h >= end; h--) {
      promises.push(
        rpc('getblockhash', [h]).then(function (hash) { return rpc('getblock', [hash]); }).catch(function () { return null; })
      );
    }
    Promise.all(promises).then(function (blocks) {
      var valid = blocks.filter(function (b) { return b !== null; });
      valid.sort(function (a, b) { return b.height - a.height; });
      var html = '';
      valid.forEach(function (b) {
        html += '<tr class="clickable-block" data-height="' + b.height + '" style="cursor:pointer">' +
          '<td class="mono">' + b.height + '</td>' +
          '<td class="mono" style="font-size:11px;color:var(--accent)" title="' + b.hash + '">' + shortAddr(b.hash) + '</td>' +
          '<td><span class="addr-short" title="' + b.miner + '">' + shortAddr(b.miner) + '</span></td>' +
          '<td class="mono">' + (b.tx_count || 0) + '</td>' +
          '<td>' + timeAgo(b.time) + '</td></tr>';
      });
      if (html) tbody.innerHTML = html;
    });
  }

  window.explorerPrev = function () { if (state.explorerPage > 1) { state.explorerPage--; updateExplorerBlocks(); } };
  window.explorerNext = function () { state.explorerPage++; updateExplorerBlocks(); };
  window.explorerSearch = function () {
    var val = el('search-block-input').value.trim();
    if (val) openBlockModal(val);
  };

  /* BLOCK MODAL */

  window.closeBlockModal = function () { el('block-modal').classList.remove('active'); };

  window.openBlockModal = function (identifier) {
    el('block-modal').classList.add('active');
    el('modal-block-body').innerHTML = '<div style="text-align:center;padding:40px;color:var(--text-muted)">Loading...</div>';
    el('modal-block-title').textContent = 'Block ' + identifier;

    var heightNum = parseInt(identifier);
    var promise;
    if (!isNaN(heightNum)) {
      promise = rpc('getblockhash', [heightNum]).then(function (hash) { return rpc('getblock', [hash]); });
    } else {
      promise = rpc('getblock', [identifier]);
    }

    promise.then(function (b) {
      var reward = calculateReward(b.height);
      var html = '<table class="data-table" style="margin-bottom:20px"><tbody>' +
        '<tr><td>Hash</td><td class="mono" style="word-break:break-all;font-size:11px;color:var(--accent)">' + b.hash + '</td></tr>' +
        '<tr><td>Height</td><td class="mono">' + b.height + '</td></tr>' +
        '<tr><td>Time</td><td class="mono">' + new Date(b.time * 1000).toLocaleString() + '</td></tr>' +
        '<tr><td>Miner</td><td class="mono" style="word-break:break-all;font-size:11px">' + b.miner + '</td></tr>' +
        '<tr><td>Reward</td><td class="mono">' + knots2kot(reward) + ' KOT</td></tr>' +
        '<tr><td>Nonce</td><td class="mono">' + b.nonce + '</td></tr>' +
        '</tbody></table>';

      html += '<div class="card-title mb-16" style="margin-top:24px;border-top:1px solid var(--border);padding-top:16px;">Transactions (' + (b.tx_count || 0) + ')</div>';

      if (b.transactions && b.transactions.length > 0) {
        html += '<table class="data-table"><thead><tr><th>Sender</th><th>Recipient</th><th>Amount</th><th>Fee</th></tr></thead><tbody>';
        b.transactions.forEach(function (tx) {
          html += '<tr>' +
            '<td class="mono" style="font-size:10px" title="' + (tx.sender || '') + '">' + shortAddr(tx.sender || '') + '</td>' +
            '<td class="mono" style="font-size:10px" title="' + (tx.recipient || '') + '">' + shortAddr(tx.recipient || '') + '</td>' +
            '<td class="mono">' + knots2kot(tx.amount || 0) + ' KOT</td>' +
            '<td class="mono">' + knots2kot(tx.fee || 0) + '</td></tr>';
        });
        html += '</tbody></table>';
      } else {
        html += '<div style="text-align:center;color:var(--text-muted);padding:20px">No transactions in this block (coinbase only).</div>';
      }
      el('modal-block-body').innerHTML = html;
    })
    .catch(function (err) {
      el('modal-block-body').innerHTML = '<div style="text-align:center;color:var(--red);padding:20px">Error: ' + err.message + '</div>';
    });
  };

  /* NETWORK VIZ (D3) */

  var vizSvg = null, vizSim = null;

  function updateNetworkViz(miners) {
    if (typeof d3 === 'undefined' || !miners || miners.length === 0) return;
    var container = el('network-viz');
    var w = container.clientWidth, h = container.clientHeight;
    if (w === 0 || h === 0) return;

    if (!vizSvg) {
      container.querySelectorAll('svg').forEach(function (s) { s.remove(); });
      vizSvg = d3.select('#network-viz').append('svg').attr('width', w).attr('height', h);
      vizSvg.append('defs').append('marker')
        .attr('id', 'arrow').attr('viewBox', '0 0 10 10')
        .attr('refX', 20).attr('refY', 5)
        .attr('markerWidth', 6).attr('markerHeight', 6).attr('orient', 'auto')
        .append('path').attr('d', 'M 0 0 L 10 5 L 0 10 z').attr('fill', 'rgba(0,212,170,0.4)');
      vizSvg.append('g').attr('class', 'links');
      vizSvg.append('g').attr('class', 'nodes');
    }

    var addrMap = {}, newNodes = [], links = [];
    var oldNodes = vizSim ? vizSim.nodes() : [];

    miners.forEach(function (m, i) {
      var addr = m.address || '';
      addrMap[addr] = i;
      var existing = oldNodes.find(function (n) { return n.id === addr; });
      newNodes.push({
        id: addr,
        balance: m.balance_knots || 0,
        blocks: m.blocks_mined || 0,
        lastMined: m.last_mined_height || 0,
        activity: getActivity(m),
        referrer: m.referrer || null,
        x: existing ? existing.x : w / 2 + (Math.random() - 0.5) * 100,
        y: existing ? existing.y : h / 2 + (Math.random() - 0.5) * 100,
        vx: existing ? existing.vx : 0,
        vy: existing ? existing.vy : 0
      });
    });

    miners.forEach(function (m) {
      var addr = m.address || '';
      var ref = m.referrer;
      if (ref && addrMap[ref] !== undefined && addrMap[addr] !== undefined) {
        links.push({ source: addr, target: ref });
      }
    });

    if (!vizSim) {
      vizSim = d3.forceSimulation()
        .force('link', d3.forceLink().id(function (d) { return d.id; }).distance(80))
        .force('charge', d3.forceManyBody().strength(-50))
        .force('center', d3.forceCenter(w / 2, h / 2))
        .force('x', d3.forceX(w / 2).strength(0.05))
        .force('y', d3.forceY(h / 2).strength(0.05))
        .force('collision', d3.forceCollide().radius(22).iterations(2));

      vizSim.on('tick', function () {
        vizSvg.select('g.links').selectAll('line')
          .attr('x1', function (d) { return cl(d.source.x, 10, w - 10); })
          .attr('y1', function (d) { return cl(d.source.y, 10, h - 10); })
          .attr('x2', function (d) { return cl(d.target.x, 10, w - 10); })
          .attr('y2', function (d) { return cl(d.target.y, 10, h - 10); });
        vizSvg.select('g.nodes').selectAll('circle')
          .attr('cx', function (d) { return d.x = cl(d.x, 10, w - 10); })
          .attr('cy', function (d) { return d.y = cl(d.y, 10, h - 10); });
      });
    }

    vizSim.nodes(newNodes);
    vizSim.force('link').links(links);
    vizSim.alpha(0.3).restart();

    var linkSel = vizSvg.select('g.links').selectAll('line')
      .data(links, function (d) { return (d.source.id || d.source) + '-' + (d.target.id || d.target); });
    linkSel.exit().remove();
    linkSel.enter().append('line')
      .attr('stroke', 'rgba(0,212,170,0.25)').attr('stroke-width', 1.5)
      .attr('marker-end', 'url(#arrow)');

    var nodeSel = vizSvg.select('g.nodes').selectAll('circle')
      .data(newNodes, function (d) { return d.id; });
    nodeSel.exit().remove();
    var nodeEnter = nodeSel.enter().append('circle')
      .attr('stroke-width', 2).attr('fill-opacity', 0.3).attr('cursor', 'pointer')
      .on('mouseover', function (ev, d) { showTooltip(ev, d); })
      .on('mouseout', function () { el('viz-tooltip').style.display = 'none'; })
      .on('click', function (ev, d) {
        navigator.clipboard.writeText(d.id).then(function () { toast('Address copied', 'success'); });
      })
      .call(d3.drag()
        .on('start', function (ev, d) { if (!ev.active) vizSim.alphaTarget(0.3).restart(); d.fx = d.x; d.fy = d.y; })
        .on('drag', function (ev, d) { d.fx = ev.x; d.fy = ev.y; })
        .on('end', function (ev, d) { if (!ev.active) vizSim.alphaTarget(0); d.fx = null; d.fy = null; })
      );

    nodeEnter.merge(nodeSel)
      .attr('r', function (d) { return Math.max(5, Math.min(16, 4 + Math.sqrt(d.blocks) * 2)); })
      .attr('fill', function (d) { return actColor(d.activity); })
      .attr('stroke', function (d) { return actColor(d.activity); });
  }

  function cl(v, min, max) { return Math.max(min, Math.min(max, v)); }

  function getActivity(m) {
    var blocks = m.blocks_mined || 0;
    var last = m.last_mined_height || 0;
    if (blocks === 0) return 'inactive';
    if (state.height - last <= 2880) return 'active';
    if (state.height - last <= 8640) return 'recent';
    return 'idle';
  }

  function actColor(a) {
    return a === 'active' ? '#00d4aa' : a === 'recent' ? '#4da6ff' : a === 'idle' ? '#ff9f43' : '#555a6e';
  }

  function showTooltip(ev, d) {
    var tt = el('viz-tooltip');
    tt.innerHTML = '<div class="tt-addr">' + shortAddr(d.id) + '</div>' +
      '<div class="tt-row"><span>Balance</span><span class="tt-val">' + knots2kot(d.balance) + ' KOT</span></div>' +
      '<div class="tt-row"><span>Blocks</span><span class="tt-val">' + d.blocks + '</span></div>' +
      '<div class="tt-row"><span>Status</span><span class="tt-val">' + d.activity + '</span></div>' +
      (d.referrer ? '<div class="tt-row"><span>Referrer</span><span class="tt-val">' + shortAddr(d.referrer) + '</span></div>' : '');
    tt.style.display = 'block';
    var rect = el('network-viz').getBoundingClientRect();
    var x = ev.clientX - rect.left + 12, y = ev.clientY - rect.top - 10;
    if (x + 200 > rect.width) x -= 220;
    tt.style.left = x + 'px'; tt.style.top = y + 'px';
  }

  /* WALLET */

  window.showSendPanel = function () {
    el('send-panel').classList.remove('hidden');
    el('receive-panel').classList.add('hidden');
    estimateFee();
  };
  window.hideSendPanel = function () { el('send-panel').classList.add('hidden'); };
  window.showReceivePanel = function () {
    el('receive-panel').classList.remove('hidden');
    el('send-panel').classList.add('hidden');
    el('receive-address').textContent = state.address || '';
  };
  window.hideReceivePanel = function () { el('receive-panel').classList.add('hidden'); };

  function estimateFee() {
    rpc('estimatefee', [5400])
      .then(function (fee) {
        var rec = fee.recommended_fee_knots || 1;
        el('send-fee').textContent = knots2kot(rec) + ' KOT (' + rec + ' knots)';
      })
      .catch(function () { el('send-fee').textContent = knots2kot(1) + ' KOT (1 knot min)'; });
  }

  window.executeSend = function () {
    var to = el('send-to').value.trim();
    var amount = parseFloat(el('send-amount').value);
    if (!to) { toast('Enter a recipient address', 'error'); return; }
    if (!amount || amount <= 0) { toast('Enter a valid amount', 'error'); return; }
    if (!state.mnemonic) { toast('Wallet not loaded', 'error'); return; }

    el('btn-send').disabled = true;
    el('btn-send').textContent = 'Sending...';

    rpc('wallet_send', [state.mnemonic, to, amount, null])
      .then(function (r) {
        toast('Tx broadcast: ' + shortAddr(r.txid || ''), 'success');
        el('send-to').value = ''; el('send-amount').value = '';
        hideSendPanel();
        state.lastTxHistoryHeight = -1;
        pollData();
      })
      .catch(function (err) { toast('Send failed: ' + err.message, 'error'); })
      .finally(function () { el('btn-send').disabled = false; el('btn-send').textContent = 'Confirm Send'; });
  };

  /* MINING */

  window.toggleMining = function () {
    if (state.isMining) stopMining(); else startMining();
  };

  function startMining() {
    var threads = parseInt(el('thread-input').value) || 2;
    threads = Math.min(8, Math.max(1, threads));
    var ref = localStorage.getItem('knotcoin_referrer');
    var args = ref ? [state.mnemonic, threads, ref] : [state.mnemonic, threads];
    rpc('start_mining', args)
      .then(function () {
        state.isMining = true;
        state.miningThreads = threads;
        state.miningStart = Date.now();
        toast('Mining started (' + threads + ' threads)', 'success');
        updateMiningUI();
      })
      .catch(function (err) { toast('Start failed: ' + err.message, 'error'); });
  }

  function stopMining() {
    rpc('stop_mining')
      .then(function () {
        state.isMining = false;
        updateMiningUI();
        toast('Mining stopped', 'success');
      })
      .catch(function (err) { toast('Stop failed: ' + err.message, 'error'); });
  }

  function updateMiningTick() {
    if (!state.isMining) return;
    var elapsed = Math.floor((Date.now() - state.miningStart) / 1000);
    el('mining-uptime').textContent = formatUptime(elapsed);

    if (state.miningBlocksFound > 0 && elapsed > 0) {
      var blocksPerSec = state.miningBlocksFound / elapsed;
      var maxVal = hexToFloat(MAX_TARGET_HEX);
      var curVal = hexToFloat(state.difficulty);
      if (curVal > 0) {
        var diff = maxVal / curVal;
        var localRate = blocksPerSec * diff * 4294967296;
        el('my-hashrate').textContent = formatHashrate(localRate);
      }
    }
  }

  function updateMiningUI() {
    var s = el('mining-state'), btn = el('btn-toggle-mining');
    if (state.isMining) {
      s.textContent = 'Mining (' + state.miningBlocksFound + ' blocks found)';
      s.className = 'mining-state active';
      btn.textContent = 'Stop Mining'; btn.className = 'btn btn-danger';
    } else {
      s.textContent = 'Idle'; s.className = 'mining-state inactive';
      btn.textContent = 'Start Mining'; btn.className = 'btn btn-primary';
      el('mining-uptime').textContent = '0s';
      el('my-hashrate').textContent = '0 H/s';
    }
    el('my-blocks').textContent = state.miningBlocksFound;
    el('my-earnings').textContent = knots2kot(state.balance) + ' KOT';
  }

  /* REFERRALS */

  function updateReferrals() {
    if (!state.address) return;
    rpc('getreferralinfo', [state.address])
      .then(function (info) {
        // Use privacy code instead of full address
        var privacyCode = info.privacy_code || state.address.substring(0, 16);
        el('referral-link').textContent = 'knotcoin:?ref=' + privacyCode;
        el('ref-total').textContent = info.total_referred_miners || 0;
        el('ref-earned').textContent = (info.total_referral_bonus_kot || '0.00000000') + ' KOT';

        var active = 0, inactive = 0;
        var tbody = el('referral-list');
        var html = '';
        state.miners.forEach(function (m) {
          if (m.referrer === state.address) {
            var act = getActivity(m);
            if (act === 'active' || act === 'recent') active++; else inactive++;
            html += '<tr><td class="addr-short" title="' + m.address + '">' + shortAddr(m.address) + '</td>' +
              '<td><span class="badge badge-' + (act === 'active' ? 'green' : act === 'recent' ? 'blue' : 'red') + '">' + act + '</span></td>' +
              '<td class="mono">' + (m.blocks_mined || 0) + '</td>' +
              '<td class="mono accent">5%</td></tr>';
          }
        });
        el('ref-active').textContent = active;
        el('ref-inactive').textContent = inactive;
        if (html) tbody.innerHTML = html;
      })
      .catch(function () { });
  }

  window.copyReferral = function () {
    console.log('📋 copyReferral called, address:', state.address);
    if (!state.address) {
      console.warn('⚠️ No wallet address available');
      toast('Please unlock your wallet first', 'error');
      return;
    }
    
    // Get the current text from the referral link element
    var linkEl = el('referral-link');
    var linkText = linkEl ? linkEl.textContent : null;
    
    if (linkText && linkText !== '—' && linkText.startsWith('knotcoin:?ref=')) {
      // Copy the displayed link directly
      console.log('📋 Copying displayed link:', linkText);
      navigator.clipboard.writeText(linkText)
        .then(function () { 
          console.log('✅ Referral link copied successfully');
          toast('Referral link copied', 'success'); 
        })
        .catch(function (err) {
          console.error('❌ Clipboard write failed:', err);
          toast('Failed to copy: ' + err.message, 'error');
        });
    } else {
      // Fallback: fetch from RPC
      console.log('📋 Fetching privacy code from RPC...');
      rpc('getreferralinfo', [state.address])
        .then(function (info) {
          var privacyCode = info.privacy_code || state.address.substring(0, 16);
          var link = 'knotcoin:?ref=' + privacyCode;
          console.log('📋 Copying link from RPC:', link);
          navigator.clipboard.writeText(link)
            .then(function () { 
              console.log('✅ Referral link copied successfully');
              toast('Referral link copied', 'success'); 
            });
        })
        .catch(function (err) {
          console.error('❌ RPC failed, using fallback:', err);
          // Fallback to address if RPC fails
          var link = 'knotcoin:?ref=' + state.address;
          navigator.clipboard.writeText(link)
            .then(function () { 
              console.log('✅ Referral link copied (fallback)');
              toast('Referral link copied', 'success'); 
            });
        });
    }
  };

  /* GOVERNANCE */

  function updateGovernance() {
    if (!state.address) return;
    rpc('getgovernanceinfo', [state.address])
      .then(function (info) {
        el('gov-weight').textContent = (info.governance_weight_pct || '0.00%');
        el('gov-cap').textContent = info.cap_pct || '10.00%';
        var b = state.miners.find(function (m) { return m.address === state.address; });
        if (b) el('gov-blocks').textContent = (b.blocks_mined || 0);
      })
      .catch(function () { });

    rpc('getmininginfo').then(function (info) {
      if (info.ponc_rounds) el('param-rounds').textContent = info.ponc_rounds;
    }).catch(function () { });
  }

  window.showNewProposal = function () { el('new-proposal-form').classList.remove('hidden'); };
  window.hideNewProposal = function () { el('new-proposal-form').classList.add('hidden'); };

  window.submitProposal = function () {
    var param = el('proposal-param').value;
    var value = el('proposal-value').value.trim();
    if (!value) { toast('Enter a proposed value', 'error'); return; }

    var govData = Array.from(new TextEncoder().encode(param + ':' + value));
    var buf = new Uint8Array(32);
    for (var i = 0; i < 32 && i < govData.length; i++) buf[i] = govData[i];
    var govHex = Array.from(buf).map(function (b) { return b.toString(16).padStart(2, '0'); }).join('');

    rpc('wallet_send', [state.mnemonic, state.address, 0.0001, govHex])
      .then(function () { toast('Proposal submitted', 'success'); hideNewProposal(); })
      .catch(function (err) { toast('Proposal failed: ' + err.message, 'error'); });
  };

  /* SETTINGS */

  window.revealMnemonic = function () {
    var mn = el('settings-mnemonic'), btn = el('btn-reveal-mnemonic'), cpBtn = el('btn-copy-mnemonic');
    if (mn.classList.contains('hidden')) {
      mn.textContent = state.mnemonic || 'No mnemonic stored';
      mn.classList.remove('hidden');
      btn.textContent = 'Hide Mnemonic';
      if (cpBtn) cpBtn.classList.remove('hidden');
    } else {
      mn.classList.add('hidden');
      btn.textContent = 'Reveal Mnemonic';
      if (cpBtn) cpBtn.classList.add('hidden');
    }
  };

  window.copyMnemonic = function () {
    if (state.mnemonic) {
      navigator.clipboard.writeText(state.mnemonic)
        .then(function () { toast('Mnemonic copied to clipboard', 'success'); });
    }
  };

  window.copyAddress = function () {
    if (state.address) {
      navigator.clipboard.writeText(state.address)
        .then(function () { toast('Address copied', 'success'); });
    }
  };

  window.deleteWallet = function () {
    alert('🗑️ Delete wallet function called!');
    console.log('🗑️ Delete wallet button clicked');
    
    if (!confirm('⚠️ DELETE WALLET?\n\nThis will permanently delete:\n• Your mnemonic phrase\n• Your wallet address\n• All wallet data from this device\n\nYou will NOT be able to recover your funds without a backup!\n\nAre you absolutely sure?')) {
      alert('❌ Delete cancelled by user');
      console.log('❌ Delete cancelled');
      return;
    }
    
    alert('⚠️ FINAL WARNING - Click OK to delete permanently');
    
    if (!confirm('FINAL WARNING!\n\nThis action cannot be undone.\n\nClick OK to permanently delete your wallet.')) {
      alert('❌ Delete cancelled at final warning');
      console.log('❌ Delete cancelled (final warning)');
      return;
    }
    
    alert('🗑️ Deleting wallet data now...');
    console.log('🗑️ Deleting wallet data...');
    
    // Stop polling
    if (state.pollTimer) {
      clearInterval(state.pollTimer);
      state.pollTimer = null;
      console.log('🔄 Stopped polling');
    }
    
    // Clear ALL localStorage
    try {
      alert('🗑️ Clearing localStorage...');
      localStorage.clear();
      console.log('🗑️ Cleared localStorage');
      alert('✅ localStorage cleared successfully');
    } catch (e) {
      alert('❌ Error clearing localStorage: ' + e.message);
      console.error('❌ Error clearing localStorage:', e);
    }
    
    // Clear state
    state.mnemonic = null;
    state.address = null;
    state.balance = 0;
    state.referral = { privacyCode: '', totalReferred: 0, totalBonus: 0 };
    state.isMining = false;
    console.log('🗑️ Cleared state');
    
    alert('🔄 Reloading page in 2 seconds...');
    console.log('✅ Wallet deleted - reloading...');
    
    // Force reload
    setTimeout(function () {
      console.log('🔄 Reloading page now...');
      alert('🔄 Reloading NOW!');
      window.location.href = window.location.href;
    }, 2000);
  };

  window.showDebugInfo = function () {
    var panel = el('debug-panel');
    var output = el('debug-output');
    
    if (!panel.classList.contains('hidden')) {
      panel.classList.add('hidden');
      el('btn-show-debug').textContent = 'Show Debug Info';
      return;
    }
    
    panel.classList.remove('hidden');
    el('btn-show-debug').textContent = 'Hide Debug Info';
    
    var html = '<div style="color: #00d4aa; margin-bottom: 12px; font-weight: bold;">🔍 KNOTCOIN DEBUG PANEL</div>';
    
    // localStorage
    html += '<div style="color: #4da6ff; margin-top: 12px;">📦 localStorage:</div>';
    html += '<div style="margin-left: 16px;">';
    html += 'Mnemonic: ' + (localStorage.getItem('knotcoin_mnemonic') ? '✅ EXISTS' : '❌ NONE') + '<br/>';
    html += 'Address: ' + (localStorage.getItem('knotcoin_address') || '❌ NONE') + '<br/>';
    html += 'Referrer: ' + (localStorage.getItem('knotcoin_referrer') || 'NONE') + '<br/>';
    html += '</div>';
    
    // State
    html += '<div style="color: #4da6ff; margin-top: 12px;">📊 State:</div>';
    html += '<div style="margin-left: 16px;">';
    html += 'Connected: ' + (state.connected ? '✅ YES' : '❌ NO') + '<br/>';
    html += 'Address: ' + (state.address || '❌ NONE') + '<br/>';
    html += 'Balance: ' + knots2kot(state.balance) + ' KOT<br/>';
    html += 'Block Height: ' + state.height + '<br/>';
    html += 'Polling: ' + (state.pollTimer ? '✅ RUNNING' : '❌ STOPPED') + '<br/>';
    html += 'Mining: ' + (state.isMining ? '✅ ACTIVE' : '❌ INACTIVE') + '<br/>';
    html += '</div>';
    
    // Elements
    html += '<div style="color: #4da6ff; margin-top: 12px;">🎨 Elements:</div>';
    html += '<div style="margin-left: 16px;">';
    html += 'Onboarding: ' + (el('onboarding') ? '✅ EXISTS' : '❌ MISSING') + '<br/>';
    html += 'Onboarding Hidden: ' + (el('onboarding') && el('onboarding').classList.contains('hidden') ? 'YES' : 'NO') + '<br/>';
    html += 'App: ' + (el('app') ? '✅ EXISTS' : '❌ MISSING') + '<br/>';
    html += 'App Hidden: ' + (el('app') && el('app').classList.contains('hidden') ? 'YES' : 'NO') + '<br/>';
    html += 'Delete Button: ' + (el('action-btn-delete-wallet') ? '✅ EXISTS' : '❌ MISSING') + '<br/>';
    html += 'Balance Element: ' + (el('wallet-balance') ? '✅ EXISTS' : '❌ MISSING') + '<br/>';
    html += '</div>';
    
    // Functions
    html += '<div style="color: #4da6ff; margin-top: 12px;">⚙️ Functions:</div>';
    html += '<div style="margin-left: 16px;">';
    html += 'deleteWallet: ' + (typeof deleteWallet === 'function' ? '✅ EXISTS' : '❌ MISSING') + '<br/>';
    html += 'rpc: ' + (typeof rpc === 'function' ? '✅ EXISTS' : '❌ MISSING') + '<br/>';
    html += 'pollData: ' + (typeof pollData === 'function' ? '✅ EXISTS' : '❌ MISSING') + '<br/>';
    html += '</div>';
    
    // Quick Actions
    html += '<div style="color: #4da6ff; margin-top: 12px;">⚡ Quick Actions:</div>';
    html += '<div style="margin-left: 16px;">';
    html += '<button onclick="testRPC()" style="margin: 4px; padding: 4px 8px; cursor: pointer;">Test RPC</button>';
    html += '<button onclick="testBalance()" style="margin: 4px; padding: 4px 8px; cursor: pointer;">Test Balance</button>';
    html += '<button onclick="forceDelete()" style="margin: 4px; padding: 4px 8px; cursor: pointer; background: #ff6b6b;">Force Delete</button>';
    html += '</div>';
    
    output.innerHTML = html;
  };

  window.testRPC = function () {
    rpc('getblockcount')
      .then(function (h) { alert('✅ RPC Working! Block height: ' + h); })
      .catch(function (e) { alert('❌ RPC Error: ' + e.message); });
  };

  window.testBalance = function () {
    if (!state.address) { alert('❌ No address in state'); return; }
    rpc('getbalance', [state.address])
      .then(function (b) { alert('✅ Balance: ' + knots2kot(b.balance_knots || 0) + ' KOT'); })
      .catch(function (e) { alert('❌ Balance Error: ' + e.message); });
  };

  window.forceDelete = function () {
    if (!confirm('Force delete wallet? This will clear everything immediately.')) return;
    localStorage.clear();
    alert('✅ localStorage cleared. Reloading...');
    window.location.reload();
  };

  /* CLICK-TO-COPY */

  document.addEventListener('click', function (e) {
    if (e.target.classList.contains('addr-short') || e.target.classList.contains('balance-addr')) {
      var addr = e.target.getAttribute('title') || e.target.textContent;
      navigator.clipboard.writeText(addr).then(function () { toast('Copied: ' + shortAddr(addr)); });
    }
  });

  /* EVENT BINDINGS */

  function bindEvents() {
    function bind(id, fn) { var e = document.getElementById(id); if (e) e.addEventListener('click', fn); }

    bind('action-btn-1', function () { showOnboard('referral'); });
    bind('action-btn-2', function () { showOnboard('import'); });
    bind('btn-confirm-create', function () { confirmCreateWallet(); });
    bind('action-btn-15', function () { showOnboard('create'); });
    bind('action-btn-16', function () { showOnboard('welcome'); });
    bind('action-btn-4', function () { confirmImportWallet(); });
    bind('action-btn-5', function () { showOnboard('welcome'); });
    bind('btn-explorer-prev', function () { explorerPrev(); });
    bind('btn-explorer-next', function () { explorerNext(); });
    bind('btn-search-block', function () { explorerSearch(); });
    bind('action-btn-6', function () { showSendPanel(); });
    bind('action-btn-7', function () { showReceivePanel(); });
    bind('action-btn-8', function () { hideSendPanel(); });
    bind('btn-send', function () { executeSend(); });
    bind('action-btn-9', function () { hideReceivePanel(); });
    bind('action-btn-10', function () { copyAddress(); });
    bind('btn-toggle-mining', function () { toggleMining(); });
    bind('referral-link', function () { copyReferral(); });
    bind('action-btn-11', function () { showNewProposal(); });
    bind('action-btn-12', function () { hideNewProposal(); });
    bind('action-btn-13', function () { submitProposal(); });
    bind('btn-reveal-mnemonic', function () { revealMnemonic(); });
    bind('btn-copy-mnemonic', function () { copyMnemonic(); });
    bind('action-btn-delete-wallet', function () { deleteWallet(); });
    bind('btn-show-debug', function () { showDebugInfo(); });

    var searchInput = document.getElementById('search-block-input');
    if (searchInput) searchInput.addEventListener('keyup', function (e) { if (e.key === 'Enter') explorerSearch(); });

    var handleBlockClick = function (e) {
      var tr = e.target.closest('tr.clickable-block');
      if (tr) openBlockModal(tr.getAttribute('data-height'));
    };
    var rb = document.getElementById('recent-blocks');
    if (rb) rb.addEventListener('click', handleBlockClick);
    var eb = document.getElementById('explorer-blocks');
    if (eb) eb.addEventListener('click', handleBlockClick);

    bind('btn-close-modal', function () { closeBlockModal(); });

    var walletAddr = document.getElementById('wallet-address');
    if (walletAddr) walletAddr.addEventListener('click', function () { copyAddress(); });
  }

  /* INIT */

  function init() {
    console.log('🚀 Initializing Knotcoin...');
    bindEvents();
    initNav();

    // Check localStorage
    state.mnemonic = localStorage.getItem('knotcoin_mnemonic');
    state.address = localStorage.getItem('knotcoin_address');
    
    console.log('📦 localStorage check:');
    console.log('  Mnemonic:', state.mnemonic ? 'EXISTS' : 'NONE');
    console.log('  Address:', state.address || 'NONE');

    loadAuth().then(function () {
      return waitForNode();
    }).then(function () {
      // Only enter app if BOTH mnemonic AND address exist
      if (state.mnemonic && state.address) {
        console.log('✅ Wallet found, entering app...');
        enterApp();
      } else {
        console.log('ℹ️ No wallet, showing onboarding...');
        // Make sure onboarding is visible
        el('onboarding').classList.remove('hidden');
        el('app').classList.add('hidden');
      }
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

})();
