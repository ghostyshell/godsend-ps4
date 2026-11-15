# Use the virtual controller

The Controller page is a DualShock 4 rendered in the app. It sends input to
the console over a WebRTC data channel.

## Set up

1. Install Control4Free once: Homebrew, then Install on its card. It also
   works launched from the console's own homebrew menu.
2. Open Controller. The header shows "service ready" when the payload answers
   on port 4264.
3. Pick a controller slot (one to four players can be driven).
4. Choose Connect. The pad lights up when the channel opens.

## Using it

- Click or tap and hold the buttons. Face buttons, d-pad, shoulders, share,
  options, PS and the touchpad click are all wired.
- Drag the discs for the sticks; they spring back to centre on release.
- The L2 and R2 sliders are analog, 0 to 255.
- Keyboard: arrows for the d-pad, J cross, K circle, L square, I triangle,
  Q and E for L1 and R1, Shift and Control for L3 and R3, Enter for options,
  Backspace for share.

## How it works

The app's webview owns the peer connection: it creates the offer, keeps two
negotiated data channels (control on id 0, input on id 1) and streams the
input message at 60 Hz. The native side only posts the offer, because the
payload refuses a request whose `Origin` is not its own or `null`, and a
webview always sends one. That is why the exchange runs in Rust while the
channels run in the page.

If the channel drops, the page says so and closes the pad. Reconnect after
the console finishes loading.

## Limits

- The payload must be running. There is no built-in PS4 pad API.
- Video is not streamed. This is input only, for a console connected to a TV.
- Motion sensors, rumble and touch coordinates beyond the click are not sent.
