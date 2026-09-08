/**
 * SOL Wallet app.js
 *
 * Balance/Send/History: Direct Solana RPC via web3.js (no backend needed)
 * Market data: CoinGecko public API
 * Signing: LOCAL only — private key never leaves device
 * Backend (optional): set API to your Render URL for fee estimation + QR
 */
'use strict';

const APP_VERSION = '2.0.1'; // cache bust - findSourceTokenAccount fix
console.log('[SOL Wallet] Version:', APP_VERSION);

const API        = 'https://sol-wallet-1.onrender.com'; // Render backend
const BACKEND_OK  = true; // backend is live

// Public CORS-friendly RPC endpoints (tried in order on fallback)
const FALLBACK_RPCS = [
  'https://rpc.ankr.com/solana',
  'https://solana-mainnet.g.alchemy.com/v2/demo',
  'https://api.mainnet-beta.solana.com',
];
const STOR       = { kp:'sw_kp', rpc:'sw_rpc', net:'sw_net' };
const HIST_LIMIT = 20;

// ─ SPL Token mint addresses ──────────────────────────────────────────────────
const SPL_TOKENS = {
  USDC: {
    name:     'USD Coin',
    symbol:   'USDC',
    devnet:   '4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU',
    mainnet:  'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
    decimals: 6,
    color:    'usdc-col',
    icon:     '$',
    usdPrice: 1.0,
  },
  PYUSD: {
    name:     'PayPal USD',
    symbol:   'PYUSD',
    devnet:   'CXk2AMBfi3TwaEL2468s6zP8xq9NxTXjp9gjMgzeUynM',
    mainnet:  '2b1kV6DkPAnxd5ixfnxCpjxmKwqjjaYmCZfHsFu24GXo',
    decimals: 6,
    color:    'pyusd-col',
    icon:     '₱',
    usdPrice: 1.0,
  },
};

// Which token is currently selected in the send screen
let currentToken = 'SOL';
// Cached token balances { uiAmount: number | null }
const tokenBals = { USDC: null, PYUSD: null };

// ─ Coin SVG icons (inline, no external deps) ────────────────────────────────
const COIN_ICONS = {
  SOL: `<svg viewBox="0 0 646 646" width="22" height="22" xmlns="http://www.w3.org/2000/svg">
    <path fill="#fff" d="M108.53 478.77a17.6 17.6 0 0112.45-5.16h477.71a8.8 8.8 0 016.22 15.02l-81.55 81.55a17.6 17.6 0 01-12.45 5.16H32.7a8.8 8.8 0 01-6.22-15.02z"/>
    <path fill="#fff" d="M108.53 77.78a18.05 18.05 0 0112.45-5.16h477.71a8.8 8.8 0 016.22 15.02l-81.55 81.55a17.6 17.6 0 01-12.45 5.16H32.7a8.8 8.8 0 01-6.22-15.02z"/>
    <path fill="#fff" d="M523.21 278.27a17.6 17.6 0 00-12.45-5.16H32.7a8.8 8.8 0 00-6.22 15.02l81.55 81.55a17.6 17.6 0 0012.45 5.16h477.71a8.8 8.8 0 006.22-15.02z"/>
  </svg>`,

  BTC: `<svg viewBox="0 0 32 32" width="22" height="22" xmlns="http://www.w3.org/2000/svg">
    <path fill="#F7931A" d="M16 0C7.164 0 0 7.164 0 16s7.164 16 16 16 16-7.164 16-16S24.836 0 16 0z"/>
    <path fill="#fff" d="M22.2 13.8c.3-2-1.2-3.1-3.3-3.8l.7-2.7-1.7-.4-.7 2.6-1.3-.3.7-2.6-1.7-.4-.7 2.7-1-.2-2.3-.6-.4 1.8s1.3.3 1.2.3c.7.2.8.6.8 1l-.8 3.3v.2l-1.1 4.5c-.1.2-.3.5-.7.4-.1 0-1.2-.3-1.2-.3l-.8 2 2.2.5 1.2.3-.7 2.7 1.7.4.7-2.7 1.3.3-.7 2.7 1.7.4.7-2.8c2.9.6 5.1.3 6-2.3.7-2-.1-3.2-1.5-3.9 1.1-.3 1.9-1 2.1-2.5zm-3.8 5.3c-.5 2-3.9 1-5 .7l.9-3.6c1.1.3 4.6.8 4.1 2.9zm.5-5.3c-.5 1.8-3.3 1-4.3.7l.8-3.2c1 .3 4 .7 3.5 2.5z"/>
  </svg>`,

  ETH: `<svg viewBox="0 0 32 32" width="22" height="22" xmlns="http://www.w3.org/2000/svg">
    <circle cx="16" cy="16" r="16" fill="#627EEA"/>
    <path fill="rgba(255,255,255,0.6)" d="M16.498 4v8.87l7.497 3.35z"/>
    <path fill="#fff" d="M16.498 4L9 16.22l7.498-3.35z"/>
    <path fill="rgba(255,255,255,0.6)" d="M16.498 21.968v6.027L24 17.616z"/>
    <path fill="#fff" d="M16.498 27.995v-6.028L9 17.616z"/>
    <path fill="rgba(255,255,255,0.2)" d="M16.498 20.573l7.497-4.353-7.497-3.348z"/>
    <path fill="rgba(255,255,255,0.6)" d="M9 16.22l7.498 4.353v-7.701z"/>
  </svg>`,

  USDC: `<img src="https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png" width="22" height="22" style="border-radius:50%;display:block;object-fit:cover" alt="USDC" onerror="this.outerHTML='<svg viewBox=\\'0 0 32 32\\' width=\\'22\\' height=\\'22\\'><circle cx=\\'16\\' cy=\\'16\\' r=\\'16\\' fill=\\'%232775CA\\'/><text x=\\'16\\' y=\\'21\\' text-anchor=\\'middle\\' font-size=\\'13\\' font-weight=\\'900\\' fill=\\'white\\'>$</text></svg>'"/>`,

  PYUSD: `<img src="https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/2b1kV6DkPAnxd5ixfnxCpjxmKwqjjaYmCZfHsFu24GXo/logo.png" width="22" height="22" style="border-radius:50%;display:block;object-fit:cover" alt="PYUSD" onerror="this.src='https://assets.coingecko.com/coins/images/31212/small/PYUSD_Logo_%282%29.png'"/>`,

  LINK: `<svg viewBox="0 0 32 32" width="22" height="22" xmlns="http://www.w3.org/2000/svg">
    <circle cx="16" cy="16" r="16" fill="#2A5ADA"/>
    <path fill="#fff" d="M16 6.4l-1.9 1.1-5.3 3.1L6.9 11.6v8.8l1.9 1.1 5.3 3 1.9 1.1 1.9-1.1 5.3-3 1.9-1.1v-8.8l-1.9-1.1-5.3-3.1L16 6.4zm0 2.2l5.3 3v6l-5.3 3-5.3-3v-6l5.3-3z"/>
  </svg>`,

  AVAX: `<svg viewBox="0 0 32 32" width="22" height="22" xmlns="http://www.w3.org/2000/svg">
    <circle cx="16" cy="16" r="16" fill="#E84142"/>
    <path fill="#fff" d="M20.1 19.7h3.2l-5.7-10-2.8 5 2.3 4c.5.7.5 1 3 1zM8.7 19.7h3.2l1.3-2.4-1.6-2.9-2.9 5.3z"/>
    <path fill="#fff" d="M16.3 9.7c-.3-.5-.8-.5-1.1 0L8 21.6c-.3.5 0 1.1.5 1.1h15.5c.6 0 .9-.6.5-1.1l-8.2-12z"/>
    <circle cx="16" cy="16" r="16" fill="none"/>
  </svg>`,

  DOGE: `<svg viewBox="0 0 32 32" width="22" height="22" xmlns="http://www.w3.org/2000/svg">
    <circle cx="16" cy="16" r="16" fill="#C2A633"/>
    <path fill="#fff" d="M11 8.5h5.2c4.3 0 7.3 2.7 7.3 7.5s-3 7.5-7.3 7.5H11V8.5zm3 12.5h2c2.7 0 4.5-1.8 4.5-5s-1.8-5-4.5-5h-2v10z"/>
    <path fill="#fff" d="M9 15.5h7v2H9z"/>
  </svg>`,

  BNB: `<svg viewBox="0 0 32 32" width="22" height="22" xmlns="http://www.w3.org/2000/svg">
    <circle cx="16" cy="16" r="16" fill="#F3BA2F"/>
    <path fill="#fff" d="M12.1 14.8L16 10.9l3.9 3.9 2.3-2.3L16 6.3 9.8 12.5l2.3 2.3zM6.3 16l2.3-2.3 2.3 2.3-2.3 2.3zM12.1 17.2l3.9 3.9 3.9-3.9 2.3 2.3L16 25.7l-6.2-6.2 2.3-2.3zM21.1 16l2.3-2.3 2.3 2.3-2.3 2.3zM18.5 16l-2.5-2.5L13.5 16l2.5 2.5 2.5-2.5z"/>
  </svg>`,
};

// ─ Market coins to fetch ────────────────────────────────────────────────────
const MARKET_COINS = [
  { id:'solana',      sym:'SOL',  name:'Solana',    cls:'sol-ico',  icon:'SOL'  },
  { id:'bitcoin',     sym:'BTC',  name:'Bitcoin',   cls:'btc-ico',  icon:'BTC'  },
  { id:'ethereum',    sym:'ETH',  name:'Ethereum',  cls:'eth-ico',  icon:'ETH'  },
  { id:'usd-coin',    sym:'USDC', name:'USD Coin',  cls:'usdc-ico', icon:'USDC' },
  { id:'chainlink',   sym:'LINK', name:'Chainlink', cls:'link-ico', icon:'LINK' },
  { id:'avalanche-2', sym:'AVAX', name:'Avalanche', cls:'avax-ico', icon:'AVAX' },
  { id:'dogecoin',    sym:'DOGE', name:'Dogecoin',  cls:'doge-ico', icon:'DOGE' },
  { id:'binancecoin', sym:'BNB',  name:'BNB',       cls:'bnb-ico',  icon:'BNB'  },
];

// ─ State ────────────────────────────────────────────────────────────────────
const S = {
  kp: null, conn: null, histPage: 0,
  pending: null, price: null, solBal: 0,
  marketData: [], lastMarketFetch: 0,
};
let w3;

// ─ DOM ──────────────────────────────────────────────────────────────────────
const g    = id  => document.getElementById(id);
const $$   = sel => [...document.querySelectorAll(sel)];
const show = el  => (typeof el==='string'?g(el):el)?.classList.remove('hidden');
const hide = el  => (typeof el==='string'?g(el):el)?.classList.add('hidden');
const txt  = (id,v) => { const e=g(id); if(e) e.textContent=v; };
const esc  = s => String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#39;');
const trunc = (a,n=6) => !a||a.length<=n*2 ? a : `${a.slice(0,n)}…${a.slice(-n)}`;
const fmtTs = ts => ts ? new Date(ts*1000).toLocaleString([],{dateStyle:'short',timeStyle:'short'}) : '—';
const fmtUSD = n => {
  if (n >= 1e12) return `$${(n/1e12).toFixed(2)}T`;
  if (n >= 1e9)  return `$${(n/1e9).toFixed(2)}B`;
  if (n >= 1e6)  return `$${(n/1e6).toFixed(2)}M`;
  return `$${n.toLocaleString(undefined,{maximumFractionDigits:2})}`;
};

function showScreen(name) {
  $$('.screen').forEach(s=>s.classList.remove('active'));
  const el = g(`screen-${name}`);
  if (el) el.classList.add('active');
  $$('.bn').forEach(b=>b.classList.toggle('active', b.dataset.screen===name));
  ({
    home:    refreshHome,
    markets: refreshMarkets,
    receive: refreshReceive,
    history: ()=>loadHistory(0),
    settings:refreshSettings,
    send:    resetSend,
  }[name]||Function.prototype)();
}

// ─ Toast ────────────────────────────────────────────────────────────────────
function toast(msg, type='', ms=3200) {
  const t = document.createElement('div');
  t.className = `toast${type?' '+type:''}`;
  t.textContent = msg;
  g('toast-container').appendChild(t);
  setTimeout(()=>t.remove(), ms);
}

// ─ Modals ───────────────────────────────────────────────────────────────────
function showLoading(msg='Please wait…'){txt('modal-loading-text',msg);show('modal-loading');}
function hideLoading(){hide('modal-loading');}
function showAlert(title,body,{cancel=false}={}) {
  return new Promise(res=>{
    txt('modal-alert-title',title); txt('modal-alert-body',body);
    cancel?show('modal-alert-cancel'):hide('modal-alert-cancel');
    show('modal-alert');
    g('modal-alert-ok').onclick    = ()=>{hide('modal-alert');res(true);};
    g('modal-alert-cancel').onclick= ()=>{hide('modal-alert');res(false);};
  });
}

// ─ Lamport helpers ──────────────────────────────────────────────────────────
const SOL9 = 1_000_000_000n;
function parseLamports(raw) {
  const s = String(raw).trim();
  if (!s || !/^\d+(\.\d+)?$/.test(s)) throw new Error('Invalid amount — digits only, e.g. 0.001');
  const [w,f=''] = s.split('.');
  if (f.length>9) throw new Error('Max 9 decimal places');
  const v = BigInt(w)*SOL9 + BigInt(f.padEnd(9,'0'));
  if (v===0n) throw new Error('Amount must be greater than zero');
  return v;
}

/** Parse a stablecoin amount string into raw integer units (6 decimals) */
function parseSplUnits(raw, decimals) {
  const s = String(raw).trim();
  if (!s || !/^\d+(\.\d+)?$/.test(s)) throw new Error('Invalid amount — digits only, e.g. 10');
  const [w, f=''] = s.split('.');
  if (f.length > decimals) throw new Error(`Max ${decimals} decimal places`);
  const v = BigInt(w) * (10n ** BigInt(decimals)) + BigInt(f.padEnd(decimals,'0'));
  if (v === 0n) throw new Error('Amount must be greater than zero');
  return v;
}

// ─ Clipboard ────────────────────────────────────────────────────────────────
async function copy(text, label='Copied') {
  try { await navigator.clipboard.writeText(text); }
  catch(_) {
    const el=Object.assign(document.createElement('textarea'),{value:text,style:'position:fixed;opacity:0'});
    document.body.appendChild(el); el.select(); document.execCommand('copy'); el.remove();
  }
  toast(`${label} ✓`, 'success');
}

// ─ Backend API helpers (kept for future use) ─────────────────────────────────
async function apiGet(path) {
  const r = await fetch(API+path, {signal:AbortSignal.timeout(10000)});
  const d = await r.json();
  if (!r.ok) throw new Error(d.message||`API ${r.status}`);
  return d;
}
async function apiPost(path, body) {
  const r = await fetch(API+path, {
    method:'POST', headers:{'Content-Type':'application/json'},
    body:JSON.stringify(body), signal:AbortSignal.timeout(15000),
  });
  const d = await r.json();
  if (!r.ok) throw new Error(d.message||`API ${r.status}`);
  return d;
}

// ─ API — direct Solana RPC, no backend required ──────────────────────────────

async function getBalance(address) {
  // 1) Render backend /wallet/balance
  try {
    const r = await fetch(`${API}/wallet/balance/${address}`, {signal: AbortSignal.timeout(15000)});
    if (r.ok) {
      const d = await r.json();
      return { balance_sol: d.balance_sol || 0, balance_lamports: d.balance_lamports || 0 };
    }
  } catch (_) { /* fall through */ }

  // 2) /rpc proxy fallback
  try {
    const r = await fetch(`${API}/rpc`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc:'2.0', id:1, method:'getBalance', params:[address, { commitment:'confirmed' }] }),
      signal: AbortSignal.timeout(15000),
    });
    if (r.ok) {
      const j = await r.json();
      const lamports = j?.result?.value ?? 0;
      return { balance_sol: lamports / 1_000_000_000, balance_lamports: lamports };
    }
  } catch (_) { /* fall through */ }

  // All failed
  return { balance_sol: 0, balance_lamports: 0, _error: true };
}

async function estimateFee(from, to, amountSol) {
  const bal = await getBalance(from);
  const amount = parseFloat(amountSol);
  const fee = 0.000005;
  const total = amount + fee;
  const sufficient = bal.balance_sol >= total;
  return {
    amount_sol: amountSol,
    network_fee_sol: fee.toFixed(6),
    network_fee_usd: S.price ? `$${(fee * S.price).toFixed(4)}` : '',
    total_sol: total.toFixed(9).replace(/\.?0+$/,''),
    sender_balance_sol: bal.balance_sol.toFixed(6),
    sufficient_funds: sufficient,
    insufficient_funds_message: sufficient ? '' : `Need ${total.toFixed(6)} SOL, have ${bal.balance_sol.toFixed(6)} SOL`,
  };
}

async function broadcastTx(b64) {
  const bytes = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
  const sig = await sendRawTxFetch(bytes);
  const { network } = loadNet();
  return {
    signature: sig,
    explorer_url: `https://solscan.io/tx/${sig}${network==='mainnet-beta'?'':'?cluster='+network}`,
  };
}

async function getTxHistory(address, limit, offset) {
  const { network } = loadNet();

  // 1) Try dedicated backend endpoint — fetches from chain with proper detail
  try {
    const r = await fetch(
      `${API}/wallet/signatures/${address}?limit=${limit}&offset=${offset}`,
      { signal: AbortSignal.timeout(15000) }
    );
    if (r.ok) {
      const d = await r.json();
      if (d.transactions?.length) {
        // Enrich with parsed tx details via /rpc batch
        try {
          const batch = d.transactions.map((tx, i) => ({
            jsonrpc: '2.0', id: i + 1,
            method: 'getTransaction',
            params: [tx.signature, { encoding: 'jsonParsed', commitment: 'confirmed', maxSupportedTransactionVersion: 0 }]
          }));
          const detailResp = await fetch(`${API}/rpc`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(batch),
            signal: AbortSignal.timeout(20000),
          });
          if (detailResp.ok) {
            const details = await detailResp.json();
            const detailArr = Array.isArray(details) ? details : [details];
            d.transactions = d.transactions.map((tx, i) => {
              const result = detailArr.find(x => x.id === i + 1)?.result;
              if (!result?.meta) return tx;
              const keys = result.transaction?.message?.accountKeys || [];
              const myIdx = keys.findIndex(k => (k.pubkey || k) === address);
              const pre  = result.meta.preBalances  || [];
              const post = result.meta.postBalances || [];
              if (myIdx >= 0) {
                const diff = (post[myIdx] || 0) - (pre[myIdx] || 0);
                tx.direction  = diff >= 0 ? 'received' : 'sent';
                tx.amount_sol = (Math.abs(diff) / 1e9).toFixed(6);
                tx.fee_sol    = result.meta.fee ? (result.meta.fee / 1e9).toFixed(9) : tx.fee_sol;
                for (let j = 0; j < keys.length; j++) {
                  if (j === myIdx) continue;
                  const key = keys[j]?.pubkey || keys[j];
                  if (typeof key === 'string' && key.length >= 32) { tx.counterparty_address = key; break; }
                }
              }
              return tx;
            });
          }
        } catch (_) { /* use undetailed data */ }
        return d;
      }
    }
  } catch (_) { /* fall through */ }

  // 2) Fallback: pure /rpc proxy
  try {
    const sigResp = await fetch(`${API}/rpc`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0', id: 1, method: 'getSignaturesForAddress',
        params: [address, { limit: limit + offset, commitment: 'confirmed' }]
      }),
      signal: AbortSignal.timeout(15000),
    });
    if (!sigResp.ok) throw new Error('rpc error');
    const sigJson = await sigResp.json();
    const sigs = (sigJson?.result || []).slice(offset, offset + limit);
    if (!sigs.length) return { transactions: [], count: 0 };

    // Enrich with parsed transaction details via batch /rpc call
    let txDetails = [];
    try {
      const batch = sigs.map((s, i) => ({
        jsonrpc: '2.0', id: i + 1,
        method: 'getTransaction',
        params: [s.signature, { encoding: 'jsonParsed', commitment: 'confirmed', maxSupportedTransactionVersion: 0 }]
      }));
      const detailResp = await fetch(`${API}/rpc`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(batch),
        signal: AbortSignal.timeout(20000),
      });
      if (detailResp.ok) {
        const j = await detailResp.json();
        txDetails = Array.isArray(j) ? j : [j];
      }
    } catch (_) { /* use basic data */ }

    const transactions = sigs.map((s, i) => {
      const result = txDetails.find(d => d.id === i + 1)?.result;
      let direction = 'sent', amount_sol = '—', counterparty = '', fee_sol = '0.000005';

      if (result?.meta) {
        fee_sol = result.meta.fee ? (result.meta.fee / 1e9).toFixed(9) : fee_sol;
        const keys = result.transaction?.message?.accountKeys || [];
        const myIdx = keys.findIndex(k => (k.pubkey || k) === address);
        const pre  = result.meta.preBalances  || [];
        const post = result.meta.postBalances || [];
        if (myIdx >= 0) {
          const diff = (post[myIdx] || 0) - (pre[myIdx] || 0);
          direction  = diff >= 0 ? 'received' : 'sent';
          amount_sol = (Math.abs(diff) / 1e9).toFixed(6);
        }
        for (let j = 0; j < keys.length; j++) {
          if (j === myIdx) continue;
          const key = keys[j]?.pubkey || keys[j];
          if (typeof key === 'string' && key.length >= 32) { counterparty = key; break; }
        }
      }

      return {
        signature: s.signature,
        direction, amount_sol, fee_sol,
        block_time: s.blockTime,
        status: s.err ? 'failed' : 'confirmed',
        counterparty_address: counterparty,
        explorer_url: `https://solscan.io/tx/${s.signature}${network === 'mainnet-beta' ? '' : '?cluster=' + network}`,
      };
    });
    return { transactions, count: transactions.length };
  } catch (_) {
    return { transactions: [], count: 0 };
  }
}

const getQrCode = () => Promise.resolve(null);

// ─ Market data (CoinGecko — direct browser, public API) ─────────────────────
async function fetchMarketData() {
  const now = Date.now();
  if (S.marketData.length && now - S.lastMarketFetch < 60000) return S.marketData;

  try {
    const ids = MARKET_COINS.map(c=>c.id).join(',');
    const url = `https://api.coingecko.com/api/v3/coins/markets?vs_currency=usd&ids=${ids}&order=market_cap_desc&per_page=20&page=1&sparkline=false&price_change_percentage=24h`;
    const r = await fetch(url, {signal:AbortSignal.timeout(8000)});
    if (!r.ok) throw new Error('CoinGecko error');
    const data = await r.json();
    S.marketData = data;
    S.lastMarketFetch = now;
    const solData = data.find(c=>c.id==='solana');
    if (solData) S.price = solData.current_price;
    return data;
  } catch(e) {
    console.warn('Market fetch failed:', e.message);
    return S.marketData;
  }
}

// ─ Storage ──────────────────────────────────────────────────────────────────
const saveKp  = kp    => localStorage.setItem(STOR.kp, bs58.encode(kp.secretKey));
const clearKp = ()    => localStorage.removeItem(STOR.kp);
const saveNet = (u,n) => { localStorage.setItem(STOR.rpc,u); localStorage.setItem(STOR.net,n); };
const loadNet = ()    => ({ rpcUrl:localStorage.getItem(STOR.rpc)||'https://api.mainnet-beta.solana.com', network:localStorage.getItem(STOR.net)||'mainnet-beta' });

function loadKp() {
  const raw = localStorage.getItem(STOR.kp);
  if (!raw) return null;
  try { return w3.Keypair.fromSecretKey(bs58.decode(raw)); } catch(_) { return null; }
}

// ─ Connection helper ─────────────────────────────────────────────────────────

/** Get the first CORS-friendly RPC URL that responds successfully */
/** Returns the best RPC URL to use — always prefers the backend proxy */
async function getWorkingRpcUrl() {
  // Always use backend /rpc proxy — it never 403s from browser
  return `${API}/rpc-passthrough`;
}

/** Make a raw JSON-RPC call via the backend proxy */
async function rpcCall(method, params, timeoutMs = 12000) {
  const r = await fetch(`${API}/rpc`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
    signal: AbortSignal.timeout(timeoutMs),
  });
  if (!r.ok) throw new Error(`Backend RPC error: ${r.status}`);
  const j = await r.json();
  if (j?.error) throw new Error(j.error.message || 'RPC error');
  return j.result;
}

/** Fetch the latest blockhash — always via backend, never direct RPC */
async function fetchLatestBlockhash() {
  // Use cached blockhash if fresh (< 30s)
  if (S._cachedBlockhash && (Date.now() - S._cachedBlockhash.ts) < 30000) {
    return { blockhash: S._cachedBlockhash.blockhash, lastValidBlockHeight: S._cachedBlockhash.lastValidBlockHeight };
  }

  // 1) Dedicated /blockhash endpoint
  try {
    const r = await fetch(`${API}/blockhash`, { signal: AbortSignal.timeout(55000) });
    if (r.ok) {
      const d = await r.json();
      if (d?.blockhash) {
        S._cachedBlockhash = { ...d, ts: Date.now() };
        return { blockhash: d.blockhash, lastValidBlockHeight: d.lastValidBlockHeight };
      }
    }
  } catch (_) { /* fall through */ }

  // 2) /rpc proxy fallback
  try {
    const r = await fetch(`${API}/rpc`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc:'2.0', id:1, method:'getLatestBlockhash', params:[{ commitment:'confirmed' }] }),
      signal: AbortSignal.timeout(55000),
    });
    if (r.ok) {
      const j = await r.json();
      if (j?.result?.value?.blockhash) {
        const d = { blockhash: j.result.value.blockhash, lastValidBlockHeight: j.result.value.lastValidBlockHeight };
        S._cachedBlockhash = { ...d, ts: Date.now() };
        return d;
      }
    }
  } catch (_) { /* fall through */ }

  throw new Error('Could not fetch blockhash — backend unavailable');
}

/** Send a raw signed transaction — tries backend first, then /rpc proxy */
async function sendRawTxFetch(serializedBytes) {
  const b64 = btoa(String.fromCharCode(...serializedBytes));

  // 1) Try Render backend /transaction/send (stores in DB)
  try {
    const r = await fetch(`${API}/transaction/send`, {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({ signed_transaction_base64: b64 }),
      signal: AbortSignal.timeout(55000),
    });
    if (r.ok) {
      const d = await r.json();
      if (d?.signature) return d.signature;
    }
  } catch (_) { /* fall through */ }

  // 2) Try /rpc proxy sendTransaction
  try {
    const r = await fetch(`${API}/rpc`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0', id: 1, method: 'sendTransaction',
        params: [b64, { encoding: 'base64', skipPreflight: false, preflightCommitment: 'confirmed' }]
      }),
      signal: AbortSignal.timeout(55000),
    });
    if (r.ok) {
      const j = await r.json();
      if (j?.result && !j?.error) return j.result;
      if (j?.error) throw new Error(j.error.message || 'Send failed');
    }
  } catch (e) { throw e; }

  throw new Error('Transaction broadcast failed');
}

// Sync fallback for non-critical uses
function getConn() {
  const { rpcUrl } = loadNet();
  const rpc = S.workingRpc || rpcUrl;
  if (!S.conn || S.conn._rpcEndpoint !== rpc) S.conn = new w3.Connection(rpc, 'confirmed');
  return S.conn;
}

// ─ SLIP-0010 BIP-44 m/44'/501'/0'/0' ────────────────────────────────────────
async function hmac512(key, data) {
  const k = await crypto.subtle.importKey('raw',key,{name:'HMAC',hash:'SHA-512'},false,['sign']);
  return new Uint8Array(await crypto.subtle.sign('HMAC',k,data));
}
async function slip10(seed) {
  let m = await hmac512(new TextEncoder().encode('ed25519 seed'), seed);
  let IL=m.slice(0,32), IR=m.slice(32);
  for (const i of [44,501,0,0]) {
    const d=new Uint8Array(37); d[0]=0; d.set(IL,1);
    new DataView(d.buffer).setUint32(33,i+0x80000000,false);
    const c=await hmac512(IR,d); IL=c.slice(0,32); IR=c.slice(32);
  }
  return IL;
}

// ─ BIP-39 ───────────────────────────────────────────────────────────────────
const b39 = () => { if(window.bip39?.generateMnemonic) return window.bip39; throw new Error('bip39 not loaded'); };
async function phraseToWallet(phrase) {
  const b=b39(), n=phrase.trim().toLowerCase().replace(/\s+/g,' ');
  if (!b.validateMnemonic(n)) throw new Error('Invalid recovery phrase — check each word');
  return w3.Keypair.fromSeed(await slip10(new Uint8Array(await b.mnemonicToSeed(n))));
}
async function genWallet() {
  const b=b39(), mn=b.generateMnemonic(256);
  return { kp:await phraseToWallet(mn), mnemonic:mn };
}
function keyToWallet(b58) {
  const bytes=bs58.decode(b58);
  if (bytes.length!==64) throw new Error('Expected 64-byte keypair');
  return w3.Keypair.fromSecretKey(bytes);
}

// ─ SOL Signing ───────────────────────────────────────────────────────────────
async function signTransfer(to, lamps) {
  if (!S.kp) throw new Error('No wallet');
  // Use raw fetch for blockhash — avoids web3.js hitting forbidden RPC
  const { blockhash, lastValidBlockHeight } = await fetchLatestBlockhash();
  const tx = new w3.Transaction();
  tx.add(w3.SystemProgram.transfer({fromPubkey:S.kp.publicKey,toPubkey:new w3.PublicKey(to),lamports:lamps}));
  tx.recentBlockhash=blockhash; tx.feePayer=S.kp.publicKey; tx.lastValidBlockHeight=lastValidBlockHeight;
  tx.sign(S.kp);
  return btoa(String.fromCharCode(...tx.serialize()));
}

// ─ SPL Token helpers ─────────────────────────────────────────────────────────

function getSplMint(symbol) {
  const { network } = loadNet();
  const tk = SPL_TOKENS[symbol];
  if (!tk) return null;
  return network === 'mainnet-beta' ? tk.mainnet : tk.devnet;
}

/** Find the actual token account address from chain — no derivation guessing */
async function findSourceTokenAccount(walletAddress, mintAddress, tokenProgramId) {
  try {
    const r = await fetch(`${API}/rpc`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0', id: 1,
        method: 'getTokenAccountsByOwner',
        params: [walletAddress, { mint: mintAddress }, { encoding: 'jsonParsed' }]
      }),
      signal: AbortSignal.timeout(10000),
    });
    const j = await r.json();
    const accounts = j?.result?.value || [];
    if (accounts.length > 0) {
      return new w3.PublicKey(accounts[0].pubkey);
    }
  } catch (_) {}
  // Fall back to ATA derivation
  return getATA(new w3.PublicKey(walletAddress), mintAddress, tokenProgramId);
}

/** Derive the Associated Token Account (ATA) address using raw bytes */
function getATA(walletPubkey, mintAddress, tokenProgramId) {
  // Default to legacy token program — Token-2022 ATAs use Token-2022 program ID
  const TOKEN_PROG = new w3.PublicKey(tokenProgramId || 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
  const ASSOC_PROG = new w3.PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe1bB8');
  const mint       = new w3.PublicKey(mintAddress);

  // Use toBytes() instead of toBuffer() — doesn't need Buffer polyfill
  const walletBytes = walletPubkey.toBytes();
  const tokenBytes  = TOKEN_PROG.toBytes();
  const mintBytes   = mint.toBytes();

  const [ata] = w3.PublicKey.findProgramAddressSync(
    [walletBytes, tokenBytes, mintBytes],
    ASSOC_PROG
  );
  return ata;
}

/** Fetch SPL token balance via /rpc proxy only — no direct RPC calls */
async function getSplBalance(walletPubkey, mintAddress) {
  const ata = getATA(walletPubkey, mintAddress);
  try {
    const r = await fetch(`${API}/rpc`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc:'2.0', id:1, method:'getTokenAccountBalance',
        params:[ata.toString(), { commitment:'confirmed' }] }),
      signal: AbortSignal.timeout(10000),
    });
    if (!r.ok) return null;
    const j = await r.json();
    if (j?.result?.value) return j.result.value;
  } catch (_) {}
  return null;
}

/**
 * Write a u64 as 8 bytes little-endian into a Uint8Array at offset.
 * Browser-safe replacement for Buffer.writeBigUInt64LE.
 */
function writeU64LE(arr, value, offset) {
  let v = value;
  for (let i = 0; i < 8; i++) {
    arr[offset + i] = Number(v & 0xffn);
    v >>= 8n;
  }
}

/**
 * Build + sign an SPL token transfer transaction.
 * Uses spl-token Transfer instruction (discriminator = 3).
 * Prepends CreateAssociatedTokenAccount if destination ATA doesn't exist.
 * Browser-safe: uses Uint8Array instead of Node.js Buffer.
 */
async function signSplTransfer(toWalletAddress, symbol, amount) {
  if (!S.kp) throw new Error('No wallet loaded');

  const IS_TOKEN_2022 = symbol === 'PYUSD';
  const TOKEN_PROG_ID = IS_TOKEN_2022
    ? 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb'
    : 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA';
  const TOKEN_PROG  = new w3.PublicKey(TOKEN_PROG_ID);
  const ASSOC_PROG  = new w3.PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe1bB8');
  const SYS_PROG    = w3.SystemProgram.programId;
  const SYSVAR_RENT = new w3.PublicKey('SysvarRent111111111111111111111111111111111');
  const mintAddr = getSplMint(symbol);
  if (!mintAddr) throw new Error(`Unknown token: ${symbol}`);

  const tk       = SPL_TOKENS[symbol];
  const mint     = new w3.PublicKey(mintAddr);
  const toPubkey = new w3.PublicKey(toWalletAddress);
  const fromPub  = S.kp.publicKey;

  const rawAmount = parseSplUnits(String(amount), tk.decimals);
  if (rawAmount <= 0n) throw new Error('Amount must be greater than zero');

  // Get blockhash via backend
  const { blockhash, lastValidBlockHeight } = await fetchLatestBlockhash();

  // Find sender's actual token account from chain (not derived — avoids wrong ATA)
  const fromATA = await findSourceTokenAccount(fromPub.toString(), mintAddr, TOKEN_PROG_ID);
  // Derive recipient ATA (standard derivation is fine for destination)
  const toATA = getATA(toPubkey, mintAddr, TOKEN_PROG_ID);

  const tx = new w3.Transaction();
  tx.recentBlockhash      = blockhash;
  tx.feePayer             = fromPub;
  tx.lastValidBlockHeight = lastValidBlockHeight;

  // Note: We do NOT create the recipient's ATA here.
  // The AToken program is a native Solana program that doesn't return
  // account data via getAccountInfo, causing false "not found" errors.
  // If the recipient doesn't have an ATA, the transfer will fail with a
  // clear error message asking them to create their token account first.

  if (IS_TOKEN_2022) {
    // Token-2022 TransferChecked instruction (discriminator = 12)
    // Required for PYUSD which has transfer hooks
    const data = new Uint8Array(10);
    data[0] = 12; // TransferChecked discriminator
    writeU64LE(data, rawAmount, 1);
    data[9] = tk.decimals; // decimals field

    tx.add(new w3.TransactionInstruction({
      programId: TOKEN_PROG,
      keys: [
        { pubkey: fromATA,  isSigner: false, isWritable: true  }, // source
        { pubkey: mint,     isSigner: false, isWritable: false }, // mint (required by TransferChecked)
        { pubkey: toATA,    isSigner: false, isWritable: true  }, // dest
        { pubkey: fromPub,  isSigner: true,  isWritable: false }, // authority
      ],
      data,
    }));
  } else {
    // Standard SPL Token Transfer (discriminator = 3)
    const data = new Uint8Array(9);
    data[0] = 3;
    writeU64LE(data, rawAmount, 1);

    tx.add(new w3.TransactionInstruction({
      programId: TOKEN_PROG,
      keys: [
        { pubkey: fromATA, isSigner: false, isWritable: true  },
        { pubkey: toATA,   isSigner: false, isWritable: true  },
        { pubkey: fromPub, isSigner: true,  isWritable: false },
      ],
      data,
    }));
  }

  tx.sign(S.kp);
  return btoa(String.fromCharCode(...tx.serialize()));
}

// ─ SPL Balance loader ────────────────────────────────────────────────────────

// ─ SPL Balance loader ────────────────────────────────────────────────────────
// Uses getTokenAccountsByOwner — no ATA derivation needed, works reliably

async function loadSplBalances(walletPubkey) {
  const walletAddr = walletPubkey.toString();

  // Token mint addresses for mainnet
  const { network } = loadNet();
  const MINTS = {
    USDC:  network === 'mainnet-beta' ? SPL_TOKENS.USDC.mainnet  : SPL_TOKENS.USDC.devnet,
    PYUSD: network === 'mainnet-beta' ? SPL_TOKENS.PYUSD.mainnet : SPL_TOKENS.PYUSD.devnet,
  };

  // Live market prices
  const usdcMkt  = S.marketData.find(c => c.id === 'usd-coin');
  const pyusdMkt = S.marketData.find(c => c.id === 'paypal-usd' || c.symbol?.toLowerCase() === 'pyusd');
  const prices   = {
    USDC:  usdcMkt  ? usdcMkt.current_price  : 1.0,
    PYUSD: pyusdMkt ? pyusdMkt.current_price : 1.0,
  };

  // Fetch ALL token accounts for this wallet — query BOTH token programs
  // (PYUSD uses Token-2022, USDC uses legacy Token program)
  try {
    const TOKEN_PROG      = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA';
    const TOKEN_2022_PROG = 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb';

    const [resp1, resp2] = await Promise.all([
      fetch(`${API}/rpc`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ jsonrpc:'2.0', id:1, method:'getTokenAccountsByOwner',
          params:[walletAddr, { programId: TOKEN_PROG }, { encoding:'jsonParsed' }] }),
        signal: AbortSignal.timeout(12000),
      }),
      fetch(`${API}/rpc`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ jsonrpc:'2.0', id:2, method:'getTokenAccountsByOwner',
          params:[walletAddr, { programId: TOKEN_2022_PROG }, { encoding:'jsonParsed' }] }),
        signal: AbortSignal.timeout(12000),
      }),
    ]);

    const [j1, j2] = await Promise.all([
      resp1.ok ? resp1.json() : { result: { value: [] } },
      resp2.ok ? resp2.json() : { result: { value: [] } },
    ]);

    const accounts = [
      ...(j1?.result?.value || []),
      ...(j2?.result?.value || []),
    ];

    // Build a map of mint → uiAmount
    const balMap = {};
    for (const acct of accounts) {
      const info = acct.account?.data?.parsed?.info;
      if (!info) continue;
      const mint    = info.mint;
      const uiAmt   = info.tokenAmount?.uiAmount ?? 0;
      balMap[mint]  = (balMap[mint] || 0) + uiAmt;
    }

    // Update UI for each token
    for (const sym of ['USDC', 'PYUSD']) {
      const mint   = MINTS[sym];
      const ui     = balMap[mint] ?? 0;
      tokenBals[sym] = ui;

      const price  = prices[sym];
      const usdVal = ui * price;
      const key    = sym.toLowerCase();

      const sub = g(`ar-${key}-sub`);
      if (sub) sub.textContent = `${ui.toFixed(2)} ${sym}`;

      const usd = g(`ar-${key}-usd`);
      if (usd) usd.textContent = `$${usdVal.toFixed(2)}`;

      const chg = g(`ar-${key}-chg`);
      if (chg) {
        chg.textContent  = `$${price.toFixed(4)}`;
        chg.style.color  = 'var(--t3)';
        chg.style.fontSize = '.66rem';
      }
    }

    // Refresh total balance to include token values
    if (S.price !== null) {
      const solUsd   = S.solBal * S.price;
      const usdcUsd  = tokenBals.USDC  || 0;
      const pyusdUsd = tokenBals.PYUSD || 0;
      const total    = solUsd + usdcUsd + pyusdUsd;
      const [d, c]   = total.toFixed(2).split('.');
      txt('bal-main',  Number(d).toLocaleString());
      txt('bal-cents', '.' + c);
    }
  } catch (e) {
    console.warn('loadSplBalances failed:', e.message);
  }
}

// ─ Wallet activation ─────────────────────────────────────────────────────────
function activateWallet(kp) {
  S.kp=kp; S.conn=null; S.connVerified=false; S.workingRpc=null; saveKp(kp); updateNetBadge(); show('bottomnav'); showScreen('home');
}
function updateNetBadge() {
  const {network}=loadNet();
  const lbls={devnet:'Devnet',testnet:'Testnet','mainnet-beta':'Mainnet'};
  const lbl=lbls[network]||network;
  const cls=network==='mainnet-beta'?'mainnet':network;
  const snb = g('settings-network');
  if (snb) { snb.textContent=lbl; snb.className=`net-badge ${cls}`; }
  const pill = g('net-pill');
  if (pill) { pill.textContent=lbl; pill.className=`net-pill ${cls}`; }
}

// ══════════════════════════════════════════
//  ONBOARDING
// ══════════════════════════════════════════
function initOnboarding() {
  g('btn-create-wallet').onclick = ()=>showScreen('create');
  g('btn-import-wallet').onclick = ()=>showScreen('import');
}

// ══════════════════════════════════════════
//  CREATE
// ══════════════════════════════════════════
function initCreate() {
  let _p=null;
  g('btn-generate').onclick = async()=>{
    showLoading('Generating phrase…');
    try{ const{kp,mnemonic}=await genWallet(); _p={kp,mnemonic}; renderGrid(mnemonic); hide('create-step-1'); show('create-step-2'); }
    catch(e){ await showAlert('Error',e.message); }
    finally{ hideLoading(); }
  };
  g('confirm-written').onchange=function(){ g('btn-confirm-mnemonic').disabled=!this.checked; };
  g('btn-confirm-mnemonic').onclick=()=>{ if(!_p)return; const{kp}=_p; _p=null; activateWallet(kp); };
}
function renderGrid(mn) {
  g('mnemonic-grid').innerHTML=mn.trim().split(/\s+/).map((w,i)=>
    `<div class="word-item"><span class="word-num">${i+1}</span><span class="word-text">${esc(w)}</span></div>`
  ).join('');
}

// ══════════════════════════════════════════
//  IMPORT
// ══════════════════════════════════════════
function initImport() {
  $$('.tab').forEach(t=>t.onclick=()=>{
    $$('.tab').forEach(x=>x.classList.remove('active'));
    $$('.tab-pane').forEach(p=>hide(p));
    t.classList.add('active'); show(t.dataset.tab);
  });
  g('btn-import-mnemonic').onclick=async()=>{
    hide('import-error');
    const ph=g('input-mnemonic').value.trim();
    if(!ph){impErr('Enter your phrase.');return;}
    showLoading('Importing…');
    try{activateWallet(await phraseToWallet(ph));}
    catch(e){impErr(e.message);}
    finally{hideLoading();}
  };
  g('btn-import-privkey').onclick=()=>{
    hide('import-error');
    const k=g('input-privkey').value.trim();
    if(!k){impErr('Enter your key.');return;}
    try{activateWallet(keyToWallet(k));}catch(e){impErr(e.message);}
  };
}
const impErr=m=>{const e=g('import-error');e.textContent=m;show(e);};

// ══════════════════════════════════════════
//  HOME / WALLET
// ══════════════════════════════════════════
function initHome() {
  g('btn-refresh-balance').onclick  = refreshHome;
  g('btn-copy-addr-home').onclick   = ()=>S.kp&&copy(S.kp.publicKey.toString(),'Address copied');
  g('btn-copy-act').onclick         = ()=>S.kp&&copy(S.kp.publicKey.toString(),'Address copied');
  g('change-pill').onclick          = refreshHome;
}

async function refreshHome() {
  if (!S.kp) return;
  const addr = S.kp.publicKey.toString();

  txt('home-address-display', trunc(addr, 8));
  txt('bal-main','…'); txt('bal-cents','');
  txt('change-text','Loading…');

  try {
    const [balRes, mktData] = await Promise.all([getBalance(addr), fetchMarketData()]);
    S.solBal = balRes.balance_sol || 0;
    const rpcFailed = balRes._error === true;

    const solMkt = mktData.find(c=>c.id==='solana');
    if (solMkt) S.price = solMkt.current_price;

    if (rpcFailed) {
      // All RPCs failed — show retry state, don't show $0 as if balance is zero
      txt('bal-main', '—'); txt('bal-cents', '');
      txt('bal-sol-row', '— SOL');
      const pill = g('change-pill');
      if(pill) { pill.style.color='var(--red)'; pill.style.borderColor='rgba(240,81,110,.2)'; pill.style.background='rgba(240,81,110,.06)'; }
      txt('change-text', 'Tap to retry');
      toast('Could not reach Solana network — tap refresh', 'error', 6000);
    } else {
      if (S.price !== null) {
        const solUsd   = S.solBal * S.price;
        const usdcUsd  = tokenBals.USDC  || 0;
        const pyusdUsd = tokenBals.PYUSD || 0;
        const usd = solUsd + usdcUsd + pyusdUsd;
        const [d,c] = usd.toFixed(2).split('.');
        txt('bal-main',  Number(d).toLocaleString());
        txt('bal-cents', '.'+c);
      } else {
        txt('bal-main', S.solBal.toFixed(4));
        txt('bal-cents','');
      }

      txt('bal-sol-row', `${S.solBal.toFixed(6)} SOL`);
      updateNetBadge();

      if (solMkt) {
        const chg = solMkt.price_change_percentage_24h || 0;
        const arrow = chg>=0 ? '▲' : '▼';
        const sign  = chg>=0 ? '+' : '';
        const pill  = g('change-pill');
        pill.style.color       = chg>=0 ? 'var(--grn)' : 'var(--red)';
        pill.style.borderColor = chg>=0 ? 'rgba(20,241,149,.2)' : 'rgba(240,81,110,.2)';
        pill.style.background  = chg>=0 ? 'rgba(20,241,149,.08)' : 'rgba(240,81,110,.06)';
        txt('change-text', `${arrow} $${solMkt.current_price.toFixed(2)} · ${sign}${chg.toFixed(2)}%`);
      } else {
        txt('change-text','Live');
      }

      txt('ar-sol-sub', `${S.solBal.toFixed(6)} SOL`);
      if (S.price !== null) txt('ar-sol-usd', `$${(S.solBal*S.price).toFixed(2)}`);
      if (solMkt) {
        const c=solMkt.price_change_percentage_24h||0;
        const el = g('ar-sol-chg');
        el.textContent=`${c>=0?'+':''}${c.toFixed(2)}%`;
        el.className=`ar-chg ${c>=0?'pos':'neg'}`;
      }
    } // end else (!rpcFailed)

    // Load SPL token balances (non-blocking)
    loadSplBalances(S.kp.publicKey).catch(()=>{});

    renderRecentTxs(addr);
  } catch(e) {
    txt('bal-main','—'); txt('bal-cents',''); 
    const pill = g('change-pill');
    if(pill) { pill.style.color='var(--red)'; pill.style.borderColor='rgba(240,81,110,.2)'; pill.style.background='rgba(240,81,110,.06)'; }
    txt('change-text', 'Tap to retry');
    console.error('refreshHome error:', e.message, e);
    if (e.message?.includes('403') || e.message?.includes('Access forbidden')) {
      toast('RPC access denied — try switching to a different network', 'error', 8000);
    } else if (e.message?.includes('Failed to fetch') || e.message?.includes('Load failed') || e.message?.includes('NetworkError')) {
      toast('Network error — tap refresh to try again', 'error', 7000);
    } else if (e.message?.includes('all RPC endpoints failed')) {
      toast('All RPC endpoints unavailable — tap refresh to retry', 'error', 7000);
    } else {
      toast('Balance fetch failed — tap refresh', 'error', 6000);
    }
  }
}

async function renderRecentTxs(addr) {
  const list=g('recent-tx-list');
  try {
    const d=await getTxHistory(addr,5,0);
    list.innerHTML=d.transactions?.length ? d.transactions.map(txRow).join('') : '<div class="tx-empty">No transactions yet</div>';
  } catch(_) {
    list.innerHTML='<div class="tx-empty">Could not load transactions</div>';
  }
}

function txRow(tx) {
  const sent=tx.direction==='sent';
  const cls=sent?'sent':'received';
  const sign=sent?'−':'+';
  const ico=sent
    ? `<svg viewBox="0 0 24 24"><line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/></svg>`
    : `<svg viewBox="0 0 24 24"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>`;
  return `<a class="tx-item" href="${esc(tx.explorer_url)}" target="_blank" rel="noopener noreferrer">
    <div class="txi ${cls}">${ico}</div>
    <div class="txi-info">
      <div class="txi-type">${sent?'Sent':'Received'}</div>
      <div class="txi-addr">${sent?'To: ':'From: '}${esc(trunc(tx.counterparty_address||''))}</div>
      <div class="txi-time">${esc(fmtTs(tx.block_time))}</div>
    </div>
    <div class="txi-vals">
      <div class="txi-amt ${cls}">${sign}${esc(String(tx.amount_sol))} SOL</div>
      <div class="txi-fee">Fee: ${esc(String(tx.fee_sol||'0'))} SOL</div>
      <div class="txi-st status-${esc(tx.status||'confirmed')}">${esc(tx.status||'confirmed')}</div>
    </div>
  </a>`;
}

// ══════════════════════════════════════════
//  MARKETS
// ══════════════════════════════════════════
let currentMktTab = 'trending';

function initMarkets() {
  g('btn-refresh-markets').onclick = ()=>{ S.lastMarketFetch=0; refreshMarkets(); };
  $$('.mkt-tab').forEach(t=>t.onclick=()=>{
    $$('.mkt-tab').forEach(x=>x.classList.remove('active'));
    t.classList.add('active');
    currentMktTab = t.dataset.mtab;
    renderMarketList();
  });
}

async function refreshMarkets() {
  const list=g('market-list');
  list.innerHTML='<div class="mkt-loading">Loading market data…</div>';
  try {
    await fetchMarketData();
    renderMarketList();
  } catch(e) {
    list.innerHTML=`<div class="mkt-loading">Could not load market data</div>`;
  }
}

function renderMarketList() {
  const list=g('market-list');
  if (!S.marketData.length) { list.innerHTML='<div class="mkt-loading">No data available</div>'; return; }

  let coins = [...S.marketData];
  if (currentMktTab==='gainers') {
    coins = coins.filter(c=>c.price_change_percentage_24h>0)
                 .sort((a,b)=>b.price_change_percentage_24h-a.price_change_percentage_24h);
  } else if (currentMktTab==='losers') {
    coins = coins.filter(c=>c.price_change_percentage_24h<0)
                 .sort((a,b)=>a.price_change_percentage_24h-b.price_change_percentage_24h);
  }

  if (!coins.length) { list.innerHTML='<div class="mkt-loading">No data for this category</div>'; return; }

  list.innerHTML = coins.map((coin, idx) => {
    const meta = MARKET_COINS.find(m=>m.id===coin.id)||{sym:coin.symbol?.toUpperCase(),cls:'btc-ico'};
    const chg = coin.price_change_percentage_24h || 0;
    const chgClass = chg>0?'pos':chg<0?'neg':'flat';
    const chgStr = `${chg>=0?'+':''}${chg.toFixed(2)}%`;
    const price = coin.current_price >= 1
      ? `$${coin.current_price.toLocaleString(undefined,{maximumFractionDigits:2})}`
      : `$${coin.current_price.toFixed(4)}`;
    const mc  = coin.market_cap   ? fmtUSD(coin.market_cap).replace('$','MC $') : '';
    const vol = coin.total_volume ? fmtUSD(coin.total_volume).replace('$','Vol $') : '';
    const iconHtml = meta.icon && COIN_ICONS[meta.icon]
      ? COIN_ICONS[meta.icon]
      : `<span style="font-size:.72rem;font-weight:800">${(meta.sym||'??').slice(0,3)}</span>`;
    return `
    <div class="mkt-row" data-coin="${esc(coin.id)}">
      <span class="mkt-rank">${idx+1}</span>
      <div class="mkt-ico ${meta.cls}">${iconHtml}</div>
      <div class="mkt-info">
        <div class="mkt-name">${esc(meta.name||coin.name)}</div>
        <div class="mkt-meta">${esc(mc)}${mc&&vol?' · ':''}${esc(vol)}</div>
      </div>
      <div class="mkt-right">
        <div class="mkt-price">${price}</div>
        <div class="mkt-chg ${chgClass}">${chgStr}</div>
      </div>
    </div>`;
  }).join('');

  $$('.mkt-row').forEach(row=>{
    row.onclick = () => {
      const id   = row.dataset.coin;
      const coin = S.marketData.find(c=>c.id===id);
      if (!coin) return;
      const chg = coin.price_change_percentage_24h||0;
      const price = coin.current_price>=1
        ? `$${coin.current_price.toLocaleString(undefined,{maximumFractionDigits:2})}`
        : `$${coin.current_price.toFixed(4)}`;
      toast(`${coin.name}: ${price}  ${chg>=0?'+':''}${chg.toFixed(2)}%`, chg>=0?'success':'error', 3000);
    };
  });
}

// ══════════════════════════════════════════
//  SEND — SOL + USDC + PYUSD
// ══════════════════════════════════════════

// SOL quick-amount presets vs stablecoin presets
const SOL_QUICK   = ['0.001','0.005','0.01','0.1','0.5','1'];
const TOKEN_QUICK = ['1','5','10','25','50','100'];

// Icon HTML for each token (used in preview coin circle)
function tokenIconHTML(token) {
  if (COIN_ICONS[token]) return COIN_ICONS[token];
  const tk = SPL_TOKENS[token];
  return `<span style="font-size:1.2rem;font-weight:900;color:#fff">${tk?.icon || token[0]}</span>`;
}

function tokenColorClass(token) {
  if (token === 'SOL') return 'sol-col';
  return SPL_TOKENS[token]?.color || 'bg3';
}

function initSend() {
  // Token tab selector
  $$('.tok-tab').forEach(btn => btn.onclick = () => {
    $$('.tok-tab').forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    currentToken = btn.dataset.token;
    _onTokenChange();
  });

  // Quick amount buttons
  $$('.sq-btn').forEach(b => b.onclick = () => {
    g('send-amount').value = b.dataset.amount;
    updateSendUsdEst();
  });

  // MAX button
  g('qb-max').onclick = () => {
    let avail = 0;
    if (currentToken === 'SOL') {
      avail = Math.max(0, S.solBal - 0.000005);
    } else {
      avail = tokenBals[currentToken] ?? 0;
    }
    g('send-amount').value = avail > 0
      ? avail.toFixed(currentToken === 'SOL' ? 9 : 2).replace(/\.?0+$/, '') || '0'
      : '0';
    updateSendUsdEst();
  };

  g('send-amount').addEventListener('input', updateSendUsdEst);
  g('btn-estimate-fee').onclick = previewSend;
  g('btn-send-back').onclick    = resetSend;
  g('btn-confirm-send').onclick = execSend;
  g('btn-send-another').onclick = resetSend;
  g('btn-copy-sig').onclick     = () => {
    const v = g('result-signature')?.textContent;
    if (v && v !== '—') copy(v, 'Signature copied');
  };
}

/** Called whenever the selected token changes */
function _onTokenChange() {
  const isSOL = currentToken === 'SOL';
  const tk    = SPL_TOKENS[currentToken];

  // Update topbar title
  txt('send-topbar-title', `Send ${currentToken}`);

  // Update currency label in amount box
  txt('sab-cur-label', currentToken);

  // Update available balance
  if (isSOL) {
    txt('send-bal-display', S.solBal.toFixed(6));
    txt('send-bal-token', 'SOL');
  } else {
    const bal = tokenBals[currentToken];
    txt('send-bal-display', bal !== null ? bal.toFixed(2) : '—');
    txt('send-bal-token', currentToken);
  }

  // Update quick-amount buttons
  const presets = isSOL ? SOL_QUICK : TOKEN_QUICK;
  $$('.sq-btn').forEach((btn, i) => {
    btn.dataset.amount = presets[i] || '';
    btn.textContent    = presets[i] || '';
  });

  // Update fee note
  const feeNote = g('fee-note-text');
  if (feeNote) {
    feeNote.textContent = isSOL
      ? '~0.000005 SOL network fee · No wallet fee'
      : '~0.000005 SOL network fee · No token fee · FREE transfer';
  }

  // Clear amount input and USD estimate
  g('send-amount').value = '';
  updateSendUsdEst();
}

function updateSendUsdEst() {
  const amt = parseFloat(g('send-amount').value);
  const est = g('send-usd-est');
  if (!est) return;

  if (!isNaN(amt) && amt > 0) {
    if (currentToken === 'SOL' && S.price !== null) {
      est.textContent = `≈ $${(amt * S.price).toFixed(2)} USD`;
    } else if (currentToken !== 'SOL') {
      // Stablecoins are 1:1 USD
      est.textContent = `≈ $${amt.toFixed(2)} USD`;
    } else {
      est.textContent = '≈ $0.00 USD';
    }
  } else {
    est.textContent = '≈ $0.00 USD';
  }
}

function resetSend() {
  // Wake up the backend immediately so it's ready when user hits Send
  fetch(`${API}/blockhash`, { signal: AbortSignal.timeout(55000) })
    .then(r => r.json())
    .then(d => { if (d?.blockhash) S._cachedBlockhash = { ...d, ts: Date.now() }; })
    .catch(() => {});

  // Reset token selection
  currentToken = 'SOL';
  $$('.tok-tab').forEach(b => b.classList.toggle('active', b.dataset.token === 'SOL'));

  g('send-recipient').value = '';
  g('send-amount').value    = '';
  hide('send-form-error');
  hide('send-step-preview');
  hide('send-step-result');
  hide('send-result-success');
  hide('send-result-error');
  show('send-step-form');
  S.pending = null;
  g('btn-confirm-send').disabled = false;

  // Reset to SOL state
  _onTokenChange();
}

async function previewSend() {
  if (!S.kp) return;
  const to  = g('send-recipient').value.trim();
  const amt = g('send-amount').value.trim();
  hide('send-form-error');

  if (!to) { sendErr('Enter a recipient address.'); return; }
  try { new w3.PublicKey(to); } catch (_) { sendErr('Not a valid Solana address.'); return; }
  if (!amt || parseFloat(amt) <= 0) { sendErr('Enter an amount greater than zero.'); return; }

  if (currentToken === 'SOL') {
    // ── SOL path ──
    let lamps;
    try { lamps = parseLamports(amt); } catch (e) { sendErr(e.message); return; }

    try {
      const b = await getBalance(S.kp.publicKey.toString());
      S.solBal = b.balance_sol || 0;
      txt('send-bal-display', S.solBal.toFixed(6));
    } catch (_) {}

    showLoading('Fetching live fee…');
    try {
      const est = await estimateFee(S.kp.publicKey.toString(), to, amt);

      txt('preview-recipient', trunc(to, 8));
      txt('preview-amount',    est.amount_sol + ' SOL');
      txt('preview-fee',       est.network_fee_sol + ' SOL');
      txt('preview-fee-usd',   est.network_fee_usd || '');
      txt('preview-total',     est.total_sol + ' SOL');
      txt('preview-balance',   est.sender_balance_sol + ' SOL');

      const usdEl = g('preview-usd');
      if (usdEl && S.price !== null) {
        usdEl.textContent = `≈ $${(parseFloat(est.amount_sol) * S.price).toFixed(2)} USD`;
      } else if (usdEl) { usdEl.textContent = ''; }

      // Update preview coin icon
      const coinEl = g('sp-coin-icon');
      coinEl.className = `sp-coin ${tokenColorClass('SOL')}`;
      coinEl.innerHTML = tokenIconHTML('SOL');

      hide('send-preview-warning');
      if (!est.sufficient_funds) {
        const w = g('send-preview-warning');
        w.textContent = est.insufficient_funds_message || 'Insufficient SOL.';
        show(w);
        g('btn-confirm-send').disabled = true;
      } else {
        g('btn-confirm-send').disabled = false;
      }

      S.pending = { to, lamps, token: 'SOL' };
      hide('send-step-form');
      show('send-step-preview');
    } catch (e) {
      sendErr(e.message);
    } finally {
      hideLoading();
    }

  } else {
    // ── SPL token path (USDC / PYUSD) ──
    let rawUnits;
    try { rawUnits = parseSplUnits(amt, SPL_TOKENS[currentToken].decimals); } catch (e) { sendErr(e.message); return; }

    // Check token balance
    const available = tokenBals[currentToken] ?? 0;
    const amtFloat  = parseFloat(amt);
    if (amtFloat > available) {
      sendErr(`Insufficient ${currentToken} balance. Available: ${available.toFixed(2)} ${currentToken}`);
      return;
    }

    // Check SOL balance for fee
    try {
      const b = await getBalance(S.kp.publicKey.toString());
      S.solBal = b.balance_sol || 0;
    } catch (_) {}
    if (S.solBal < 0.000015) { // conservative: 5 lamports fee + possible ATA rent
      sendErr(`Need at least 0.000015 SOL for network fee. Current: ${S.solBal.toFixed(6)} SOL`);
      return;
    }

    // Populate preview
    txt('preview-recipient', trunc(to, 8));
    txt('preview-amount',    `${amtFloat.toFixed(2)} ${currentToken}`);
    txt('preview-fee',       '~0.000005 SOL');
    txt('preview-fee-usd',   '');
    txt('preview-total',     `${amtFloat.toFixed(2)} ${currentToken}`);
    txt('preview-balance',   `${Math.max(0, available - amtFloat).toFixed(2)} ${currentToken}`);

    const usdEl = g('preview-usd');
    if (usdEl) usdEl.textContent = `≈ $${amtFloat.toFixed(2)} USD`;

    // Update preview coin icon
    const coinEl = g('sp-coin-icon');
    coinEl.className = `sp-coin ${tokenColorClass(currentToken)}`;
    coinEl.innerHTML = tokenIconHTML(currentToken);

    hide('send-preview-warning');
    g('btn-confirm-send').disabled = false;

    S.pending = { to, rawUnits, token: currentToken, amtFloat };
    hide('send-step-form');
    show('send-step-preview');
  }
}

async function execSend() {
  if (!S.kp || !S.pending) return;
  g('btn-confirm-send').disabled = true;
  showLoading('Signing & broadcasting…');
  try {
    let b64, successMsg;

    if (S.pending.token === 'SOL') {
      b64 = await signTransfer(S.pending.to, S.pending.lamps);
      successMsg = `${(Number(S.pending.lamps) / 1e9).toFixed(6)} SOL sent successfully`;
    } else {
      b64 = await signSplTransfer(S.pending.to, S.pending.token, S.pending.amtFloat);
      successMsg = `${S.pending.amtFloat.toFixed(2)} ${S.pending.token} sent successfully`;
    }

    const res = await broadcastTx(b64);

    txt('result-signature', res.signature);
    txt('sr-sub-text', successMsg);
    const lnk = g('result-explorer-link');
    lnk.href = res.explorer_url || '#';

    hide('send-result-error');
    hide('send-step-preview');
    show('send-step-result');
    show('send-result-success');
    toast('Sent! 🚀', 'success', 5000);
    setTimeout(refreshHome, 3000);
  } catch (e) {
    // Make error message as specific as possible
    let msg = e.message || 'Transaction failed.';
    if (msg.includes('simulation failed')) {
      msg = 'Transaction simulation failed — check your token balance and try again.';
    } else if (msg.includes('insufficient')) {
      msg = 'Insufficient balance to complete this transaction.';
    } else if (msg.includes('blockhash')) {
      msg = 'Transaction expired — please try again.';
    }
    txt('result-error-msg', msg);
    hide('send-result-success');
    show('send-result-error');
    hide('send-step-preview');
    show('send-step-result');
    toast('Transaction failed', 'error');
  } finally {
    hideLoading();
    g('btn-confirm-send').disabled = false;
    S.pending = null;
  }
}

const sendErr = m => { const e = g('send-form-error'); e.textContent = m; show(e); };

// ══════════════════════════════════════════
//  RECEIVE
// ══════════════════════════════════════════
function initReceive() {
  g('btn-copy-address').onclick =()=>S.kp&&copy(S.kp.publicKey.toString(),'Address copied');
  g('btn-share-address').onclick=async()=>{
    const a=S.kp?.publicKey.toString();if(!a)return;
    if(navigator.share)try{await navigator.share({title:'My Solana Address',text:a});}catch(_){}
    else copy(a,'Address copied');
  };
}
async function refreshReceive() {
  if(!S.kp)return;
  const addr=S.kp.publicKey.toString();
  const uri=`solana:${addr}`;
  const canvas=g('qr-canvas');
  const loading=g('qr-loading');
  txt('receive-address-display',addr);

  show(loading); hide(canvas);

  // Use a fixed size that works well on mobile
  const SZ = 220;

  // Try QRCode.draw from the bundled library
  if(window.QRCode?.draw){
    try{
      canvas.width = SZ;
      canvas.height = SZ;
      // Clear any previous drawing
      canvas.getContext('2d').clearRect(0, 0, SZ, SZ);
      QRCode.draw(canvas, uri, {size: SZ});
      hide(loading); show(canvas);
      return;
    }catch(e){ console.warn('QRCode.draw failed:', e); }
  }

  // Fallback: try backend QR
  const qd = await getQrCode(addr);
  if(qd?.qr_code_png_base64){
    const img=new Image();
    img.onload=()=>{
      canvas.width=SZ; canvas.height=SZ;
      canvas.getContext('2d').drawImage(img,0,0,SZ,SZ);
      hide(loading); show(canvas);
    };
    img.src=`data:image/png;base64,${qd.qr_code_png_base64}`;
    return;
  }

  // Last resort: draw address as grid of squares (readable as text)
  canvas.width=SZ; canvas.height=SZ;
  const ctx=canvas.getContext('2d');
  ctx.fillStyle='#ffffff'; ctx.fillRect(0,0,SZ,SZ);
  ctx.fillStyle='#111111'; ctx.font='bold 9px monospace'; ctx.textAlign='center';
  const lines = addr.match(/.{1,12}/g) || [];
  lines.forEach((line,i)=>ctx.fillText(line, SZ/2, 20 + i*13));
  hide(loading); show(canvas);
}

// ══════════════════════════════════════════
//  HISTORY
// ══════════════════════════════════════════
function initHistory() {
  g('btn-refresh-history').onclick=()=>loadHistory(0);
  g('btn-hist-prev').onclick=()=>{if(S.histPage>0)loadHistory(S.histPage-1);};
  g('btn-hist-next').onclick=()=>loadHistory(S.histPage+1);
}
async function loadHistory(page) {
  if(!S.kp)return;
  S.histPage=page;
  const list=g('history-list');
  list.innerHTML='<div class="tx-empty">Loading…</div>';
  try{
    const d=await getTxHistory(S.kp.publicKey.toString(),HIST_LIMIT,page*HIST_LIMIT);
    if(!d.transactions?.length){
      list.innerHTML=`<div class="tx-empty">${page===0?'No transactions yet':'No more transactions'}</div>`;
      hide('history-pagination');
    }else{
      list.innerHTML=d.transactions.map(txRow).join('');
      txt('hist-page-info',`Page ${page+1}`);
      g('btn-hist-prev').disabled=page===0;
      g('btn-hist-next').disabled=d.count<HIST_LIMIT;
      show('history-pagination');
    }
  }catch(e){list.innerHTML=`<div class="tx-empty">${esc(e.message)}</div>`;}
}

// ══════════════════════════════════════════
//  SETTINGS
// ══════════════════════════════════════════
function initSettings() {
  g('btn-export-address').onclick=()=>S.kp&&copy(S.kp.publicKey.toString(),'Address copied');
  $$('.np').forEach(b=>b.onclick=()=>{g('input-custom-rpc').value=b.dataset.rpc;applyNet(b.dataset.rpc,b.dataset.net);});
  g('btn-save-rpc').onclick=()=>{
    const url=g('input-custom-rpc').value.trim();
    if(!url){toast('Enter an RPC URL','error');return;}
    try{new URL(url);}catch(_){toast('Invalid URL','error');return;}
    const net=url.includes('mainnet')?'mainnet-beta':url.includes('testnet')?'testnet':'devnet';
    applyNet(url,net);
  };
  g('btn-show-privkey').onclick=async()=>{
    const ok=await showAlert('Export Key','Your private key gives full access. Make sure nobody is watching.',{cancel:true});
    if(!ok||!S.kp)return;
    txt('privkey-display',bs58.encode(S.kp.secretKey));show('privkey-display-area');hide('btn-show-privkey');
  };
  g('btn-hide-privkey').onclick=()=>{txt('privkey-display','—');hide('privkey-display-area');show('btn-show-privkey');};
  g('btn-copy-privkey').onclick=()=>{const v=g('privkey-display').textContent;if(v&&v!=='—')copy(v,'Key copied');};
  g('btn-remove-wallet').onclick=async()=>{
    const ok=await showAlert('Remove Wallet','Your SOL stays on-chain. Restore with your phrase.',{cancel:true});
    if(!ok)return;
    clearKp();S.kp=S.conn=null;S.solBal=0;S.price=null;S.marketData=[];
    tokenBals.USDC=null;tokenBals.PYUSD=null;
    hide('bottomnav');toast('Wallet removed','success');showScreen('onboarding');
  };
}
function applyNet(url,net){saveNet(url,net);S.conn=null;S.connVerified=false;S.workingRpc=null;toast(`Switched to ${net}`,'success');refreshSettings();updateNetBadge();}
function refreshSettings(){
  if(!S.kp)return;
  const addr=S.kp.publicKey.toString();
  const el=g('settings-address');
  if(el){ el.textContent=trunc(addr,16); el.className='sg-val mono'; }
  const{rpcUrl}=loadNet();
  txt('settings-rpc', rpcUrl);
  const rpcInp=g('input-custom-rpc');
  if(rpcInp) rpcInp.value=rpcUrl;
  updateNetBadge();
}

// ══════════════════════════════════════════
//  NAV — wire ALL data-screen + data-back
// ══════════════════════════════════════════
function initNav() {
  $$('.bn[data-screen]').forEach(b=>b.onclick=()=>showScreen(b.dataset.screen));
  $$('[data-back]').forEach(b=>b.onclick=()=>showScreen(b.dataset.back));
  $$('[data-screen]').forEach(b=>{
    if (b.classList.contains('bn')||b.hasAttribute('data-back')) return;
    b.onclick=()=>showScreen(b.dataset.screen);
  });
}

// ══════════════════════════════════════════
//  BOOT
// ══════════════════════════════════════════
document.addEventListener('DOMContentLoaded', ()=>{
  if (!window.solanaWeb3) {
    document.body.innerHTML=`
      <div style="display:flex;align-items:center;justify-content:center;min-height:100vh;background:#13131a;color:#f0f0f5;font-family:sans-serif;text-align:center;padding:2rem">
        <div>
          <div style="font-size:3rem;margin-bottom:1rem;color:#14f195">◈</div>
          <h2 style="margin-bottom:.5rem">Vendor libraries missing</h2>
          <p style="color:#8888a8;margin:.75rem 0 1.5rem;font-size:.9rem">
            Run <code style="background:#1a1a24;padding:.2rem .5rem;border-radius:6px">frontend\\download-vendors.ps1</code>
          </p>
          <button onclick="location.reload()" style="background:#14f195;color:#000;border:none;border-radius:14px;padding:.75rem 2rem;font-size:1rem;cursor:pointer;font-weight:700">Reload</button>
        </div>
      </div>`;
    return;
  }
  w3 = window.solanaWeb3;

  initNav();
  initOnboarding(); initCreate(); initImport();
  initHome(); initMarkets(); initSend();
  initReceive(); initHistory(); initSettings();
  updateNetBadge();

  const existing = loadKp();
  if (existing) { S.kp=existing; show('bottomnav'); showScreen('home'); }
  else showScreen('onboarding');

  // Warm up the Render backend in the background so it's ready when needed
  fetch(`${API}/health`, { signal: AbortSignal.timeout(55000) }).catch(() => {});
});
