cargo build --release
cp ./target/release/poc_tokio ./poc

trap clean EXIT

function clean() {
  sudo iptables -D INPUT -p tcp --dport 5201 -j NFQUEUE --queue-num 0
}

nohup iperf3 -s >/dev/null 2>&1 &
sleep 1
IP_PID=$!

echo "Running control (without debugger)"
iperf3 -c 127.0.0.1

nohup sudo ./poc &
PID=$!
sleep 1
echo "Running benchmark for raw socket (pid=$PID)"
iperf3 -c 127.0.0.1
sudo kill $PID

sudo SYSTEM=nfqueue nohup ./poc &
PID=$!
sleep 1
echo "Running benchmark for nfqueue (pid=$PID)"
sudo iptables -A INPUT -p tcp --dport 5201 -j NFQUEUE --queue-num 0
iperf3 -c 127.0.0.1
sudo kill $PID

sudo kill $IP_PID
