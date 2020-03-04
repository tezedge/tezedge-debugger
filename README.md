Simple packet sniffer specifically for TezEdge project.
CLI commands are not implemented, thus project relies on convention settings.
Sniffer listens on port `9732` and it is required, that local tezedge node would run on same port.
Sniffer expect copy of `identity.json` in `./identity` folder. It is imperative, correct access rights 
granted to the built binary, for ease of use, there is `run.sh` present, which will built binary and 
grants them correct access rights.