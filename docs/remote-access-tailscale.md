# Remote access from a phone via Tailscale

The desktop app can serve its web UI to a phone over the network. The daemon and its sessions stay on
the PC; the phone gets the same tree and terminals in a browser tab, laid out for a small screen.

A paired device can type into every session, run commands and read files in your worktrees, and the
traffic is plain HTTP. Inside a Tailscale tailnet the transport is WireGuard-encrypted and the port is
invisible to anyone outside your devices, which is why this page uses Tailscale. Never expose the port
to the internet.

## One-time setup

### On the PC (the machine running the desktop app)

1. Install Tailscale and log in:
   - Windows: `winget install tailscale.tailscale`, then `tailscale login`
     (opens a browser page to authenticate).
   - Other platforms: see https://tailscale.com/download

### On the phone

1. Install the Tailscale app (App Store / Google Play).
2. Log in with the same account and turn the VPN on.

## Connecting

In the desktop app, open Settings > Remote access:

1. Turn `Remote access` on.
2. Set `Listen on` to `Tailscale`. The server then listens only on this machine's `100.x` tailnet
   address. `Local network (LAN)` listens on every interface instead, for a phone on the same Wi-Fi;
   `This machine only (localhost)` is the default.
3. `Port` is 4477 unless you change it.

The panel shows the addresses the server answers on, a pairing link and its QR code. Scan the QR code
with the phone's camera or open the link in the phone's browser. The phone stays paired until you press
`Regenerate token`, which creates a new link and disconnects every paired device. The panel lists the
devices connected right now.

## Troubleshooting

- The panel says this computer has no Tailscale address: Tailscale is not running or not logged in on
  the PC. Install it and sign in, or switch to `Local network (LAN)` if the phone is on the same Wi-Fi.
- The page doesn't load on the phone: check that the VPN is on on the phone and that `tailscale status`
  on the PC shows both devices. On Windows, allow the app through the firewall if a prompt appears.
- The phone shows a page asking for the pairing link: open the link or scan the QR code again; a device
  that tries a wrong token five times in a minute is blocked for up to a minute.

## Limits

- The PC must be awake and the desktop app running: the app serves the phone, and the sessions live in
  the daemon's PTYs on the PC. Disable sleep in the power settings if you rely on this.
- For a machine without a desktop session, the `docker` image runs `berth-server`, the same web UI
  without a window; its pairing link and QR code are in the server log.
