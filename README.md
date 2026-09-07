# ◎ SOL Wallet — Low-Fee Micro-Payments

A production-quality, **non-custodial** Solana wallet built in Rust, focused on
cheap micro-payments with complete fee transparency.

> **Every user pays their own Solana network fee.**
> There is no gas sponsorship, no relayer, and no hidden costs.

---

## Features

- Create a new wallet (24-word BIP-39 mnemonic, SLIP-0010 derivation)
- Import an existing wallet (mnemonic or base58 private key)
- View SOL balance + approximate USD equivalent
- Receive SOL (wallet address + QR code)
- Send very small amounts of SOL (micro-payments)
- **Live fee estimation** — fetches the actual current network fee before you confirm
- **Fee transparency** — shows network fee, service fee (0 SOL), and total separately
- Transaction history with Solana Explorer links
- Devnet by default; switchable to Mainnet-Beta via environment config

---

## Architecture

```
solana-low-fee-wallet/
├── crates/
│   ├── wallet-core/   # Key generation, signing, fee estimation, balance
│   ├── api/           # Axum REST API server (broadcasts signed transactions)
│   └── storage/       # SQLite transaction history (SQLx, no macros)
└── frontend/
    ├── index.html     # Single-page wallet UI
    ├── style.css      # Mobile-first dark theme
    ├── app.js         # Wallet logic — all signing happens here, client-side
    ├── qrcode.min.js  # Bundled QR generator (pure JS)
    └── vendor/        # Solana web3.js, tweetnacl, bs58, bip39 (run download-vendors.ps1)
```

**Security model:**
- Private keys are generated and stored **only in the browser** (localStorage)
- Private keys **never reach the backend API**
- Transaction signing happens in `app.js` using `@solana/web3.js`
- The API receives only the already-signed transaction bytes and broadcasts them
- The backend never signs or modifies transactions

---

## Quick Start

### 1. Prerequisites

- [Rust](https://rustup.rs/) (1.75+)
- Windows: Visual Studio C++ Build Tools (for OpenSSL vendored build)

### 2. Clone and configure

```powershell
cd "SOLANA WALLET"
copy .env.example .env
```

Edit `.env` if needed (defaults to Devnet):

```env
SOLANA_NETWORK=devnet
SOLANA_RPC_URL=https://api.devnet.solana.com
DATABASE_URL=sqlite:./wallet.db
API_HOST=127.0.0.1
API_PORT=3000
```

### 3. Build

```powershell
cargo build --release
```

### 4. Start the API server

```powershell
$env:RUST_LOG="info"
cargo run --bin solana-wallet-api
```

The API starts on `http://127.0.0.1:3000`.

### 5. Download frontend vendor libraries

```powershell
cd frontend
.\download-vendors.ps1
```

### 6. Open the wallet

Open `frontend/index.html` in your browser.
For development with CORS, serve it locally:

```powershell
# Python (if available)
python -m http.server 8080 --directory frontend

# Or use any static file server
```

Then visit `http://localhost:8080`.

---

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/wallet/balance/:address` | SOL balance + USD estimate |
| POST | `/transaction/estimate` | Live fee estimate (queries cluster) |
| POST | `/transaction/send` | Broadcast a signed transaction |
| GET | `/transaction/:signature` | Look up a transaction |
| GET | `/wallet/transactions/:address` | Transaction history |
| GET | `/wallet/qr/:address` | QR code PNG (base64) |
| GET | `/price/sol` | Current SOL/USD price |

### Example: estimate fee

```bash
curl -X POST http://localhost:3000/transaction/estimate \
  -H "Content-Type: application/json" \
  -d '{
    "from_address": "YourWalletAddress",
    "to_address":   "RecipientAddress",
    "amount_sol":   "0.001"
  }'
```

Response:
```json
{
  "amount_sol":          "0.001000000",
  "network_fee_sol":     "0.000005000",
  "network_fee_usd":     "≈ $0.00075 USD (estimate)",
  "service_fee_sol":     "0.000000000",
  "total_sol":           "0.001005000",
  "sufficient_funds":    true
}
```

### Example: send (client must sign first)

```bash
curl -X POST http://localhost:3000/transaction/send \
  -H "Content-Type: application/json" \
  -d '{"signed_transaction_base64": "<base64-signed-tx>"}'
```

---

## Running Tests

```powershell
cargo test
```

Tests cover:
- Wallet generation and mnemonic round-trip
- Address validation
- SOL ↔ lamport conversion (all arithmetic in integer lamports)
- Fee calculation and display
- Balance validation (insufficient funds detection)
- Transaction record construction and Explorer URL generation

---

## Devnet Testing

Get free devnet SOL from the faucet:

```bash
# Using Solana CLI
solana airdrop 1 <YOUR_ADDRESS> --url devnet

# Or visit https://faucet.solana.com
```

---

## Fee Model

Solana fees are **not hard-coded**. Every time you initiate a transfer:

1. The app fetches the **latest blockhash** from the cluster
2. It calls `getFeeForMessage` with the actual transaction message
3. The exact fee is displayed before you confirm

Typical fee: **0.000005 SOL** (5,000 lamports) for a simple transfer.

```
Recipient:         9xExampleWalletAddress
Amount:            0.001000000 SOL
Solana network fee: 0.000005000 SOL ≈ $0.00075 USD
Wallet service fee: 0.000000000 SOL
─────────────────────────────────────
Total:             0.001005000 SOL
```

---

## Security Notes

- Private keys are stored in browser `localStorage` — use a passphrase-encrypted
  keystore in a production deployment
- This wallet targets **Devnet by default** — do not use with real funds until
  you have completed a full security audit
- Never share your 24-word recovery phrase with anyone
- The backend API does not and cannot sign transactions on your behalf

---

## License

MIT
