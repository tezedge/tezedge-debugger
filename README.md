Tezedge Proxy
=============
Simple packet sniffer specifically for TezEdge project.
CLI commands are not implemented, thus project relies on convention settings.
Sniffer listens on port `9732` and it is required, that local tezedge node would run on same port.
Sniffer expect copy of `identity.json` in `./identity` folder. It is imperative, correct access rights 
granted to the built binary, for ease of use, there is `run.sh` present, which will built binary and 
grants them correct access rights.

FAQ
===
Resolving `Error: unable to access internet (check FAQ for more info): Did not received any response` problems.
To successfully model the proxy, some per-application routing is necessary (and hard to make right in most cases), and 
because of that, there might be many problems, why you cannot access internet from the proxy (which renders it unusable).
1. There is no internet connection. Try to check if `ping 8.8.8.8` does return any response.

2. Incorrect network interface is set. It is common to have multiple network interfaces present,
`tezedge_proxy` itself, requires to create two additional "tun" devices. To identify correct output interface,
check output of `ip route` command, which usually in first line defines default gateway 
(e.g.: `default via 192.168.1.1 dev enp34s0 proto dhcp metric 100`, which says that default interface to connect to the internet is through `enp34s0` device.).

3. Incorrect local address is set. Whilst `ip route` defines correct internet device, it does not specify local address
ut gateway address. To find correct one check `inet` value of the `ip address show dev <your interface>` 
(e.g.: `inet 192.168.1.199/24` which says your local address is `192.168.1.199`)

4. Tun addresses collides with existing settings. By default tun devices are set to operate on `10.0.0.0/32` namespace
(`10.0.0.0` - `10.0.1.255` addresses), you might already have active interface, which works on this range of addresses,
and because of that, it is not possible to correctly decide to which one to route the packets. You need to check your
interfaces assigned addresses and change the working address namespaces for the tun devices correctly.

5. Masquerading was not set correctly. Masquerading is a routing algorithm, to correctly set addresses of outgoing packets
during bridging (while sending packets from `tun1` to your network interface, packet have source address set to `tun1` 
address `10.0.1.1`, but to correctly send them, it is required to "masquerade" them under the network interface address `192.168.1.199`).
*DURING* internet connection health-checks, check if `iptables` was set correctly, you should see `MASQUERADE all -- anywhere anywhere`
rule in *POSTROUTING* chain of `iptables -L -t nat` command (this need root privileges!). If this rule is not present, file 
an issue in this repository.

6. RP_FILTER/IP_FORWARD did not took effect. To successfully route the packets, it is required to enable ip packet forwarding (IP_FORWARD)
a partially disable reverse path filtering (RP_FILTER). To check if settings were applied correctly, check output of 
`cat /proc/sys/net/ipv4/ip_forward` which should print `1` and `cat /proc/sys/net/ipv4/conf/<dev>/rp_filter` for devices
`tun0`, `tun1` and your internet interface (`enp34s0`) should all print value `2`. If all settings are correct, some systems
require to restart your network for this settings to take effect, check your distribution for this 
(e.g.: on Ubuntu this is done with command `service network-manager restart`)

7. If everything else fails, file an issue in this repository