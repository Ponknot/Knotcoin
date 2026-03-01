const fs = require('fs');
let code = fs.readFileSync('share/explorer/app.js', 'utf8');

// replace browser APIs with mocks
let window = { location: { reload: ()=>console.log("RELOAD") } };
let document = {
  readyState: 'loading',
  addEventListener: (e, cb) => { if(e === 'DOMContentLoaded') setTimeout(cb, 100) },
  getElementById: (id) => ({
    id,
    classList: { add: ()=>{}, remove: ()=>{}, contains: ()=>false },
    addEventListener: ()=>{},
    textContent: '',
    innerHTML: '',
    value: '',
    className: ''
  }),
  createElement: () => ({ style: {}, appendChild: ()=>{} }),
  querySelectorAll: () => []
};
let localStorage = { getItem:()=>'mnemonic', setItem:()=>{} };
let navigator = { clipboard: { writeText: async ()=>{} } };

// mock fetch for rpc
let fetchCalls = 0;
let fetch = async (url, opts) => {
    let body = JSON.parse(opts.body);
    fetchCalls++;
    console.log("RPC:", body.method, body.params || []);
    if (body.method === 'getmininginfo') return { json: async ()=>({result: {blocks: 10}}) };
    if (body.method === 'getpeerinfo') return { json: async ()=>({result: {peer_count: 1}}) };
    if (body.method === 'getbalance') return { json: async ()=>({result: {balance_knots: 10000}}) };
    if (body.method === 'get_all_miners') return { json: async ()=>({result: {miners: []}}) };
    if (body.method === 'getreferralinfo') return { json: async ()=>({result: {total_referred_miners: 0}}) };
    if (body.method === 'getgovernanceinfo') return { json: async ()=>({result: {}}) };
    if (body.method === 'getblockhash') return { json: async ()=>({result: "hash"}) };
    if (body.method === 'getblock') return { json: async ()=>({result: {height: 10}}) };
    return { json: async ()=>({result: {}}) };
};
let MAX_TARGET_HEX = "00000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF";
let KNOTS_PER_KOT = 100000000;

global.window = window;
global.document = document;
global.localStorage = localStorage;
global.navigator = navigator;
global.fetch = fetch;
global.MAX_TARGET_HEX = MAX_TARGET_HEX;
global.KNOTS_PER_KOT = KNOTS_PER_KOT;

try {
  eval(code);
} catch(e) {
  console.error("CRASH:", e);
}
setTimeout(()=>console.log("Finished. RPC Calls:", fetchCalls), 1500);
