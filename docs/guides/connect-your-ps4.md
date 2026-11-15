# Connect your PS4

GODsend PS4 needs GoldHEN running on the console. GoldHEN provides:

1. FTP on port **2121**, for files.
2. The remote package installer on port **12800**, for installs.
3. The bin loader on port **9090**, for payloads.

The virtual controller needs one more thing: the **Control4Free** payload,
which opens port **4264**. Install it from the Homebrew page once and it stays
available after each boot (GoldHEN auto-loads it if you add it to its payload
list).

## Find the console's address

On the PS4, open Settings, then Network, then View Connection Status. The IP
address is listed there. Reserve it on your router so it does not move.

## Add it in the app

1. Open Devices.
2. Choose Add console.
3. Enter a name and the IP. Leave FTP on 2121, installer on 12800, pad on
   4264 and bin loader on 9090 unless you changed them.
4. Leave the FTP user as `anonymous` with no password; GoldHEN accepts that.
5. Choose Save console, then Test.

A healthy console shows FTP up with a latency and its banner in the inspector,
installer ready, pad ready, and a title count.

## Discovery

Devices, then Discover, scans ports 2121 and 21 across your subnet and keeps
only hosts that answer as an FTP server. Use it when you do not know the
address.

## More than one console

Add each console and use Use to make it active. Every page, the pad and the
job queue work against the active console.

## Security

GoldHEN's FTP is plaintext. Keep the console and the computer on your own
network and do not expose these ports to the internet.
