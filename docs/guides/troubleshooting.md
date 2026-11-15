# Troubleshooting

## FTP does not answer

- GoldHEN must be running. After a reboot, load it again with the usual
  browser or USB route.
- GoldHEN's FTP listens on 2121 by default. If you changed the port, set it on
  the Devices page.
- The console and the computer must be on the same network and the router must
  not isolate clients from each other.
- Test with `curl -v ftp://192.168.1.60:2121/` or any FTP client.

## Installs do not start

- The installer runs on port 12800 and needs the console to reach your
  computer. A firewall prompt on the first install is normal; allow it.
- The app advertises the interface address that routes to the console. If the
  computer has several network interfaces, check that the console can reach
  that subnet.
- If the installer never answers, the app falls back to uploading into
  `/user/pkg`. Start the installer on the console to finish.

## The Controller says the service is offline

- Control4Free is not loaded. Install it from the Homebrew page, or launch it
  from the console's homebrew menu.
- Its port is 4264. If you changed it, set the pad port on Devices.
- A dropped channel after a game launch is normal; reconnect.

## Transfers are slow

- GoldHEN's FTP is a single connection. Large packages install much faster
  through the remote installer, which streams over plain HTTP.
- Wi-Fi is the usual bottleneck. Try a cable.
- A USB drive written by the app is slower than internal storage; that is the
  drive, not the app.

## A title does not appear in Library

- The scan reads `/user/appmeta`. If GoldHEN does not expose it, the title
  will not show even though it is installed; reinstall it with the app.
- Patches and DLC are filtered out on purpose.
- Choose Rescan after installing.
