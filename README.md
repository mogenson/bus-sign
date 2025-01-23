# MBTA Next Bus Sign

Rust application for the [Galactic
Unicorn](https://shop.pimoroni.com/products/space-unicorns?variant=40842033561683)
(RP2040 version) that shows the time until the next bus arrives.

![PXL_20250123_222450176~2](https://github.com/user-attachments/assets/7cf47cd9-8b1f-490c-a58b-ae4ca1f98b19)

Built with [embassy-rs](https://embassy.dev/) and uses the [unicorn-
graphics](https://github.com/domneedham/pimoroni-unicorn-rs) board support
crate.

It's currently hardcoded to query the MBTA 87 and 88 bus routes. The bus stop ID
and WiFi credentials are set with environmental variables (so you can't come to
my house and steal my WiFi).

## mbta-proxy.py

Unfortunatly the MBTA API requires HTTPS but only supports TLS 1.2 and the
[embedded-tls](https://github.com/drogue-iot/reqwless?tab=readme-ov-file#embedded-tls)
crate only provides TLS 1.3. Therefore, I made a `mbta-proxy.py` script to run
on some other device on the local network. It will forward on all parameters
from an HTTP GET request to an HTTPS request to `https://api-v3.mbta.com` and
reply with the response.
