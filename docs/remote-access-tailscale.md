# Remote access from a phone via Tailscale

The desktop app can serve its web UI to a phone over the network. The daemon and its sessions stay on
the PC; the phone opens them in a browser tab, in a layout of its own (see [On the phone](#on-the-phone)).

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

For HTTPS (needed by a phone browser for the microphone and Web Push), set `Listen on` to `Tailscale (HTTPS)`
instead. The server then stays on localhost and Berth runs `tailscale serve --bg --https=443
http://127.0.0.1:<port>`; the pairing link is `https://<machine>.<tailnet>.ts.net/?token=...`. HTTPS and
Serve must be enabled for the tailnet (first run prints a link to do it, shown in the panel as an
error). Berth runs `tailscale serve --https=443 off` when the server stops.

The panel shows the addresses the server answers on, a pairing link and its QR code. Scan the QR code
with the phone's camera or open the link in the phone's browser. The phone stays paired until you press
`Regenerate token`, which creates a new link and disconnects every paired device. The panel lists the
devices connected right now.

### A token per device

`Paired devices` in the same panel gives each device a token of its own. Type a device name, pick
`Read only` or `Full access` and press `Add device`: the panel shows that device's pairing link once,
to open on the new device. A read-only device sees the sessions but cannot type into them or answer a
prompt. The list shows every device with its access, including the links made by
[sharing one Session](#sharing-one-session-with-a-colleague) as `One Session, read only`, and `Revoke`
locks one device out without touching the others.

### Moving between networks

The server tells the page every address it answers on. Every 15 seconds the page checks whether a
faster route to the same machine answers, in the order local network, then Tailscale, then a public
HTTPS address, and moves to it. When the current route drops, it reconnects over the first route that
answers. A phone that leaves the office Wi-Fi keeps working over Tailscale, and comes back to the
local address when it returns.

## Sharing one Session with a colleague

A colleague can watch one Agent without getting access to anything else. With remote access on, open
the Session in the desktop app and press the share button in its header (the broadcast icon), then
`Share and copy link`. Send the copied link. Whoever opens it in a browser sees that Session's chat,
its tool calls grouped together and the names of the files changed in its Worktree, updated every few
seconds. They cannot type, answer a permission prompt, see other Sessions or open settings: the server
answers that link with this one Session and refuses everything else.

The shared part is the conversation, not the code, so the link carries no diffs unless you ask for
them. Tick `Include diffs` in the same popover and the colleague can click a changed file and read its
diff, read only. Untick it and the server stops answering diff requests for that link at once. The
choice is stored with the shared Session, so it survives a restart of the app.

Sharing is off for every Session until you turn it on, and the link keeps working until you press
`Stop sharing` in the same place, or revoke the device named after the Session in Settings > Remote
access. The colleague needs to reach the server the same way a phone does, so on Tailscale they must
be in your tailnet (or use `Local network (LAN)` on a trusted network).

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
  without a window; its pairing link and QR code are in the server log
  (`docker compose -f docker/compose.yaml logs berth`). Set `BERTH_PUBLIC_URL` to the address the phone
  reaches, such as `http://<pc address>:4477`, so the link and the QR code carry it; `berth-server
  --public-url` does the same.

## On the phone

The phone gets its own layout, with four tabs at the bottom: `Inbox`, `Sessions`, `Reviews` and
`Settings`, and a header with the screen title and status. The Inbox lists the sessions that need you,
with `Resume` and `Open` on the card of a crashed one. A session opens in its Chat view. Above it, the
model, `Checkpoints` and `Task note` buttons sit in one row that scrolls sideways, over a switch
between `Chat` and `Changes`.

The chat composer has two voice modes besides typing. Hold to talk: hold the microphone button, speak,
release to send what you said to the agent, or slide away to cancel. Hands-free: berth listens, a pause
sends your turn, the agent's reply is read aloud without its code, and it listens again until you press
`End voice`. Voice uses the speech recognition of the phone's browser, so it works only where the
browser offers it, and the browser asks for the microphone only over HTTPS.

## Notifications on the phone

With `Tailscale (HTTPS)`, open Berth on the phone, go to Settings, Remote access, and press `Enable notifications`. The browser asks for permission and Berth stores the subscription. From then on, when an Agent is waiting for you or has finished while the desktop app is not focused, the phone gets a push. A permission request shows `Allow` and `Deny` buttons in the notification where the phone supports notification actions (Chrome on Android); the answer goes back through the paired device and is typed into the Agent. A read-only device cannot answer. On iOS the web client must be added to the home screen first, and the notification only opens Berth.

The push keys live in the same keychain as the pairing tokens, under the entry `web-push`.
