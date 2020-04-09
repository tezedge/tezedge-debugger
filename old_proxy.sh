#!/bin/bash

# === INFO ===
# NoVPN
# Description: Bypass VPN tunnel for applications run through this tool.
VERSION="3.0.0"
# Author: KrisWebDev
# Requirements:  Linux with kernel > 2.6.4 (released in 2008).
#                This version is tested on Ubuntu 14.04 and 19.10 with bash.
#                Main dependencies are automatically installed.
#                Script will guide you for iptables upgrade if needed.
# Note: For security, this script will disable IPv6, even after --clean.

# === LICENSE ===
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU General Public License for more details.
#
#    You should have received a copy of the GNU General Public License
#    along with this program.  If not, see <http://www.gnu.org/licenses/>.

# === CONFIGURATION ===
# Find your real (non-VPN) interface manually with: ip route
# Guess with: ip route | grep "default via " | sed -n '/^.* dev \([-_[:alnum:]]\{1,\}\).*$/s//\1/p' | grep -v tun
real_interface="tun0"
# Warn if real interface guessed at startup doesn't match above setting
real_interface_check_warn=true

# === ADVANCED CONFIGURATION ===
cgroup_name="tzproxy" # Better keep it with purely lowercase alphabetic & underscore
net_cls_classid="0x00110011" # Anything from 0x00000001 to 0xFFFFFFFF
ip_table_fwmark="11" # Anything from 1 to 2147483647
ip_table_number="11" # Anything from 1 to 252

# === CODE ===
real_interface_gateway=`ip route | grep "dev ${real_interface}" | awk '/^default/ { print $3 }'`

# Handle options
action="command"
background=false
skip=false
check_sudo_conf=false
init_nb_args="$#"
cgroup_version=1

while [ "$#" -gt 0 ]; do
  case "$1" in
    -b|--background) background=true; shift 1;;
    -o|--outside) action="outside"; shift 1;;
    -i|--inside) action="inside"; shift 1;;
    -l|--list) action="list"; shift 1;;
    -s|--skip) skip=true; shift 1;;
    -f|--force) check_sudo_conf=true; shift 1;;
	  --sudok) sudok=true; shift 1;;
    -g|--cgroup) cgroup_name="$2"; shift 1; shift 1;;
    -2|--cgv2) cgroup_version=2; shift 1;;
    -I|--info) action="info"; shift 1;;
    -c|--clean) action="clean"; shift 1;;
    -h|--help) action="help"; shift 1;;
    -v|--version) echo "novpn v$VERSION"; exit 0;;
    -*) echo "Unknown option: $1. Try --help." >&2; exit 1;;
    *) break;; # Start of COMMAND or LIST
  esac
done

if { [ "$#" -lt 1 ] && [ "$action" == "command" ]; } || [ "$action" = "help" ] ; then
	me=`basename "$0"`
	echo -e "Usage : \e[1m$me [\e[4mOPTIONS\e[24m] [\e[4mCOMMAND\e[24m [\e[4mCOMMAND PARAMETERS\e[24m]]\e[0m"
	echo -e "   or : \e[1m$me [\e[4mOPTIONS\e[24m] { --outside | --inside } \e[4mLIST\e[24m\e[0m"
	echo -e "Run command outside the VPN tunnel interface."
	echo
	echo -e "\e[1m\e[4mOPTIONS\e[0m:"
	echo -e "\e[1m-b, --background\e[0m    Start \e[4mCOMMAND\e[24m as background process (release the shell)."
	echo -e "\e[1m-o, --outside \e[4mLIST\e[24m\e[0m  Move running process \e[4mLIST\e[24m outside tunnel. \e[1mBROKEN!\e[0m"
	echo -e "\e[1m-i, --inside \e[4mLIST\e[24m\e[0m   Move back running process \e[4mLIST\e[24m inside tunnel."
	echo -e "\e[1m-l, --list\e[0m          List processes going outside tunnel."
	echo -e "\e[1m-s, --skip\e[0m          Don't setup system config (never ask for root);\n                     just perform public routing test and run \e[4mCOMMAND\e[24m."
	echo -e "\e[1m--sudok\e[0m             Drop sudo rights with \"sudo -K\" before executing \e[4mCOMMAND\e[24m."
  echo -e "\e[1m-f, --force\e[0m         Force setup of system config (always ask for root)."
  echo -e "\e[1m-g, --cgroup \e[4mNAME\e[24m\e[0m   Specify cgroup name (cgroup v1) or relative path (cgroup v2). Default: \"$cgroup_name\"."
	echo -e "\e[1m-2, --cgv2\e[0m          Use Control Groups version 2 if supported."
	echo -e "\e[1m-I, --info\e[0m          Display debug information and exit."
	echo -e "\e[1m-c, --clean\e[0m         Move back all proceses to initial routing settings and remove system config."
	echo -e "\e[1m-v, --version\e[0m       Print this program version."
	echo -e "\e[1m-h, --help\e[0m          This help."
	echo
	echo -e "\e[1m\e[4mLIST\e[0m: List o f process ID or names separated by spaces."
	exit 1
fi

ip_table_name="$cgroup_name"

# This program can't ask for root outside terminal
if [ ! -t 1 ] && [ "$(id -u)" -ne 0 ]; then
	skip=true
fi

if [ "$skip" = true ]; then
	if [ "$action" = "clean" ]; then
		echo -e "\e[31mCan't use --skip with --clean. Aborting.\e[0m" >&2
		exit 1
	fi
	if [ "$check_sudo_conf" = true ]; then
		echo -e "\e[31mCan't use --skip with --force. Aborting.\e[0m" >&2
		exit 1
	fi
fi

# Interface check
if [ "$real_interface_check_warn" = true ]; then
  real_interface_guess="$(ip route | grep "default via " | sed -n '/^.* dev \([-_[:alnum:]]\{1,\}\).*$/s//\1/p' | grep -v tun)"
  if [ ! -z "$real_interface_guess" ] && [ "$real_interface" != "$real_interface_guess" ]; then
    echo -e "\e[93mWarning: Guessed real interface is \"$real_interface_guess\" but this script is configured to use \"$real_interface\"."
    echo -e "Manually edit the script settings if needed.\e[0m" >&2
  fi
fi

# Find/Check cgroup system folders
cgroup_v1_net_cls="/sys/fs/cgroup/net_cls"
cgroup_v2_root="/sys/fs/cgroup"
cgroup_base="/sys/fs/cgroup"
find_root(){
  # Find cgroup v2 root filesystem
  if [ "$cgroup_version" = 2 ]; then
    [ -f "$cgroup_v2_root""/cgroup.procs" ] || cgroup_v2_root="/sys/fs/cgroup/unified"
    [ -f "$cgroup_v2_root""/cgroup.procs" ] || cgroup_v2_root="$(mount -t cgroup2 | head -n1 | grep -oP '^cgroup2 on \K\S+')"
    if [ ! -f "$cgroup_v2_root""/cgroup.procs" ]; then
      echo -e "\e[31mCan't find a valid cgroup v2 mounted filesystem. Aborting.\e[0m" >&2
      exit 1
    fi
    echo "Using cgroups v2. Found valid cgroup v2 filesystem at $cgroup_v2_root"
    cgroup_base="$cgroup_v2_root"
  else
    # Find cgroup v1 net_cls folder
    [ -f "$cgroup_v1_net_cls""/cgroup.procs" ] || cgroup_v1_net_cls="$(mount -t cgroup | grep net_cls | head -n1 | grep -oP '^cgroup on \K\S+')"
    if [ ! -f "$cgroup_v1_net_cls""/cgroup.procs" ]; then
      echo -e "\e[31mCan't find a valid cgroup v1 net_cls folder. Aborting.\e[0m" >&2
      exit 1
    fi
    echo "Using cgroups v1. Found valid cgroup v1 net_cls folder at $cgroup_v1_net_cls"
    cgroup_base="$cgroup_v1_net_cls"
  fi
}

# Prepare variables
if [ "$cgroup_version" = 2 ]; then
  iptables_arg="--path $cgroup_name"
else
  iptables_arg="--cgroup $net_cls_classid"
fi


# Helper functions
# Check the presence of required system packages
check_install_package(){
	nothing_installed=1
	for package_name in "$@"
	do
		if ! dpkg -s "$package_name" &> /dev/null; then
      if [ "$package_name" = "cgroup-tools" ]; then
        if [ -x "$(command -v cgexec)" ]; then
          continue
        fi
        if [ "$(apt-cache search --names-only '^cgroup-tools$' | wc -l)" -eq 0 ]; then
          echo "cgroups-tools is not available in apt cache, falling back to cgroup-bin install"
          package_name="cgroup-bin"
        fi
      fi
			echo "Installing $package_name"
      read -p "Press enter to continue or Ctrl+C to cancel"
			sudo apt-get install "$package_name"
			nothing_installed=0
		fi
	done
	return $nothing_installed
}

check_package(){
	for package_name in "$@"
	do
		if ! dpkg -s "$package_name" &> /dev/null; then
      if [ "$package_name" = "cgroup-tools" ]; then
        # Ignore if cgexec is available (e.g. through cgroup-bin)
        if [ ! -x "$(command -v cgexec)" ]; then
          true
          return
        fi
      else
        true
        return
      fi
		fi
	done
	false
}

# Main functions
# Check/Install dependencies
install_dependencies(){
  if check_install_package cgroup-lite traceroute; then # Removed cgroup-tools
    if check_package cgroup-lite traceroute; then # Removed cgroup-tools
      echo "Required packages not properly installed. Aborting." >&2
      exit 1
    fi
  fi

  iptables_version=$(iptables --version | grep -oP "iptables v\K[0-9.]+")
  if dpkg --compare-versions "$iptables_version" "lt" "1.6"; then
    echo -e "\e[31mYou need iptables 1.6.0+. Please install manually. Aborting.\e[0m" >&2
    echo "Find latest iptables at http://www.netfilter.org/projects/iptables/downloads.html" >&2
    echo "Commands to install iptables 1.8.4:" >&2
    echo -e "\e[34msudo apt-get install dh-autoreconf bison flex
cd /tmp
curl https://netfilter.org/projects/iptables/files/iptables-1.8.4.tar.bz2 | tar xj
cd iptables-1.8.4
./configure --prefix=/usr      \\
          --sbindir=/sbin    \\
          --disable-nftables \\
          --enable-libipq    \\
          --with-xtlibdir=/lib/xtables \\
&& make  \\
&& sudo make install
iptables --version\e[0m" >&2
    exit 1
  fi
}

# Check and setup iptables - requires root even for check
iptable_checked=false
setup_config(){
#if [ "$cgroup_version" = 1 ]; then
  if [ ! -d "$cgroup_base/$cgroup_name" ]; then
    echo "Creating control group at \"$cgroup_base/$cgroup_name\"" >&2
    sudo mkdir -p "$cgroup_base/$cgroup_name"
    sudo chown -R "$USER":"`id -g -n "$USER"`" "$cgroup_base/$cgroup_name"
    check_sudo_conf=true
  fi
  cgroup_owner=`stat -c "%U" "$cgroup_base/$cgroup_name/cgroup.procs"`
  if [ "$cgroup_owner" != "`id -g -n "$USER"`" ] && [ "$EUID" -ne 0 ]; then
	   echo -e "\e[93mWARNING: Folder \"$cgroup_base/$cgroup_name/cgroup.procs\" already exists, it is owned by someone else ($cgroup_owner) and you are not root.\e[0m" >&2
  fi
  # Redundant
  #if [ -z "`lscgroup net_cls:$cgroup_name`" ] || [ `stat -c "%U" "$cgroup_v1_net_cls/${cgroup_name}/tasks"` != "$USER" ]; then
  #  echo "Creating cgroup net_cls:${cgroup_name}. User $USER will be able to move tasks in it without root permissions." >&2
  #  sudo cgcreate -t "$USER":"$USER" -a `id -g -n "$USER"`:`id -g -n "$USER"` -g net_cls:"$cgroup_name"
  #  check_sudo_conf=true
  #fi
  if [ "$cgroup_version" = 1 ] && [ `cat "$cgroup_v1_net_cls/$cgroup_name/net_cls.classid" | xargs -n 1 printf "0x%08x"` != "$net_cls_classid" ]; then
    echo "Applying net_cls class identifier $net_cls_classid to cgroup $cgroup_name" >&2
    echo "$net_cls_classid" | sudo tee "$cgroup_v1_net_cls/$cgroup_name/net_cls.classid" > /dev/null
    check_sudo_conf=true
  fi
	if ! grep -E "^${ip_table_number}\s+$ip_table_name" /etc/iproute2/rt_tables &>/dev/null; then
		if grep -E "^${ip_table_number}\s+" /etc/iproute2/rt_tables; then
			echo -e "\e[31mERROR: Table ${ip_table_number} already exists in /etc/iproute2/rt_tables with a different name than $ip_table_name.\e[0m" >&2
			exit 1
		fi
		echo "Creating ip routing table: number=$ip_table_number name=$ip_table_name" >&2
		echo "$ip_table_number $ip_table_name" | sudo tee -a /etc/iproute2/rt_tables > /dev/null
		check_sudo_conf=true
	fi
	if ! ip rule list | grep " lookup $ip_table_name" | grep " fwmark " &>/dev/null; then
		echo "Adding rule to use ip routing table $ip_table_name for packets with firewall mark $ip_table_fwmark" >&2
		sudo ip rule add fwmark "$ip_table_fwmark" table "$ip_table_name"
		check_sudo_conf=true
	fi
	if [ -z "`ip route list table "$ip_table_name" default via $real_interface_gateway dev ${real_interface} 2>/dev/null`" ]; then
		echo "Adding default route in ip routing table $ip_table_name via $real_interface_gateway dev $real_interface" >&2
		sudo ip route add default dev "$real_interface" table "$ip_table_name"
		# Useless?
		echo "Flushing ip route cache" >&2
		sudo ip route flush cache
		check_sudo_conf=true
	fi
	if [ "`cat /proc/sys/net/ipv4/conf/all/rp_filter`" != "0" ] && [ "`cat /proc/sys/net/ipv4/conf/all/rp_filter`" != "2" ]; then
		echo "Unset reverse path filtering for interface \"all\"" >&2
		echo 2 | sudo tee "/proc/sys/net/ipv4/conf/all/rp_filter" > /dev/null
		check_sudo_conf=true
	fi
	if [ "`cat /proc/sys/net/ipv4/conf/${real_interface}/rp_filter`" != "0" ] && [ "`cat /proc/sys/net/ipv4/conf/${real_interface}/rp_filter`" != "2" ]; then
		echo "Unset reverse path filtering for interface \"${real_interface}\"" >&2
		echo 2 | sudo tee "/proc/sys/net/ipv4/conf/${real_interface}/rp_filter" > /dev/null
		check_sudo_conf=true
	fi
  # Only check iptables configuration if another configuration item was missing or if the test fails, as it requires root rights
  if [ "$check_sudo_conf" = true ]; then
  	if ! sudo iptables -t mangle -C OUTPUT -m cgroup $iptables_arg -j MARK --set-mark "$ip_table_fwmark" 2>/dev/null; then
  		echo "Adding iptables MANGLE rule to set firewall mark $ip_table_fwmark on packets with class identifier $net_cls_classid" >&2
  		sudo iptables -t mangle -A OUTPUT -m cgroup $iptables_arg -j MARK --set-mark "$ip_table_fwmark"
  	fi
  	if ! sudo iptables -t nat -C POSTROUTING -m cgroup $iptables_arg -o "$real_interface" -j MASQUERADE 2>/dev/null; then
  		echo "Adding iptables NAT rule to force the packets with class identifier $net_cls_classid to exit through $real_interface" >&2
  		sudo iptables -t nat -A POSTROUTING -m cgroup $iptables_arg -o "$real_interface" -j MASQUERADE
  	fi
  	iptable_checked=true
  fi
  if [ "`cat /proc/sys/net/ipv6/conf/all/disable_ipv6`" != "1" ] || [ "`cat /proc/sys/net/ipv6/conf/""$real_interface""/disable_ipv6`" != "1" ] || [ "$check_sudo_conf" = true ]; then
  		echo "Disabling IPv6 (not supported/implemented)"
  		sudo ip -6 route add blackhole default metric 1
  		echo 1 | sudo tee "/proc/sys/net/ipv6/conf/all/disable_ipv6" > /dev/null
  fi
}

fcgexec() {
  "$@"
}

# Test if config is working, IPv4 only
test_routing(){
	#exit_ip="$(fcgexec -g net_cls:"$cgroup_name" traceroute -n -m 1 8.8.8.8 | sed -n '2{p;q}' | grep -oE '[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+' | head -1)"
  exit_ip="$(fcgexec traceroute -n -m 1 8.8.8.8 | sed -n '2{p;q}' | grep -oE '[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+' | head -1)"
  if [ -z "$exit_ip" ]; then
		# Old traceroute
		#exit_ip="$(fcgexec -g net_cls:"$cgroup_name" traceroute -m 1 8.8.8.8 | sed -n '2{p;q}' | grep -oE '[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+' | head -1)"
    exit_ip="$(fcgexec traceroute -m 1 8.8.8.8 | sed -n '2{p;q}' | grep -oE '[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+' | head -1)"
    if [ -z "$exit_ip" ]; then
			echo -e "\e[31mTest failed: Unable to determine source exit IP (found \"$exit_ip\").\e[0m" >&2
			if [ "$skip" = true ]; then
				echo -e "\e[31mYou should remove --skip option to perform setup.\e[0m" >&2
			fi
			return 0
		fi
	fi
	if [ -z "$real_interface_gateway" ]; then
		echo -e "\e[31mTest failed: Unable to determine real interface gateway IP (found \"$real_interface_gateway\").\e[0m" >&2
    return 0
	fi
	if [ "$exit_ip" == "$real_interface_gateway" ]; then
		echo -e "\e[32mTest OK. Trafic exits with IP \"$exit_ip\".\e[0m" >&2
    return 0
	else
		echo -e "\e[31mTest failed: Trafic exits with \"$exit_ip\" instead of \"$real_interface_gateway\".\e[0m" >&2
    return 1
	fi
}

# Reconfigure routing
reroute(){
	# if [ -z "$real_interface_gateway" ]; then
	#	echo -e "\e[31mCan't find default gateway of real interface \"${real_interface}\". Is it up?\e[0m" >&2
	#	echo -e "\e[31mAborting.\e[0m" >&2
	#	exit 1
	# fi

	if [ "$skip" = false ]; then
    setup_config
	fi

  # MOVE ourself, as program caller, to cgroup
  echo $$ | sudo tee "$cgroup_base/${cgroup_name}/cgroup.procs" > /dev/null

	# TEST
	test_routing
  testresult=$?
	if [ "$skip" = false ]; then
		if [ "$testresult" = false ]; then
			if [ "$iptable_checked" = false ] && [ "$skip" = false ]; then
				echo -e "Trying to setup iptables and redo test..." >&2
        check_sudo_conf=true
				setup_config
				test_routing
        testresult=$?
			fi
		fi
  fi

	if [ "$testresult" -ne 0 ]; then
		echo -e "\e[31mAborting.\e[0m" >&2
		exit 1
  fi
}

# List processes bypassing the VPN
list_outside(){
	return_status=1
	echo -e "PID""\t""CMD"
	cat "$cgroup_base/${cgroup_name}/cgroup.procs" | \
  while read task_pid
		do
			echo -e "${task_pid}""\t""`ps -p ${task_pid} -o comm=`";
			return_status=0
	done
	return $return_status
}

# Check/Install DEPENDENCIES
if [ "$action" = "command" ] || [ "$action" = "outside" ]; then
  if [ "$skip" = false ]; then
    echo "Checking/Installing dependencies" >&2
    install_dependencies
  fi
fi

# Find cgroup filesystem
find_root

# SETUP novpn routing
if [ "$action" = "command" ] || [ "$action" = "outside" ]; then
  echo "Checking/Setting up system configuration" >&2
  reroute
fi

# RUN command
if [ "$action" = "command" ]; then
	if [ "$sudok" = true ]; then
		sudo -K
	fi
	if [ "$#" -eq 0 ]; then
		echo "Error: COMMAND not provided." >&2
		exit 1
	fi
	if [ "$background" = true ]; then
		#cgexec -g net_cls:"$cgroup_name" --sticky "$@" &>/dev/null &
    fcgexec "$@" &>/dev/null &
		exit 0
	else
		#cgexec -g net_cls:"$cgroup_name" --sticky "$@"
    fcgexec "$@"
		exit $?
	fi

# List process OUTSIDE tunnel
# Exit code 0 (true) if at least 1 process is outside the tunnel
elif [ "$action" = "list" ]; then
	echo "List of processes bypassing tunnel:"
	list_outside
	exit $?

# Move process OUTSIDE tunnel
elif [ "$action" = "outside" ]; then
	exit_code=1
	for process in "$@"
	do
	    if [ "$process" -eq "$process" ] 2>/dev/null; then
			# Is integer (PID)
			echo "$process" | sudo tee "$cgroup_base/${cgroup_name}/cgroup.procs" > /dev/null
			exit_code=0
		else
			# Is process name
			pids=$(pidof "$process")
			for pid in $pids
			do
				echo "$pid" | sudo tee "$cgroup_base/${cgroup_name}/cgroup.procs" > /dev/null
				exit_code=0
			done
		fi
	done
	echo -e "\e[93mWARNING: Moving running processes outside the VPN tunnel DOES NOT WORK.\e[0m" >&2
	echo -e "\e[93mYou should start new processes and beware processes that have already opened windows: they may reuse existing PID.\e[0m" >&2
	echo "List of processes bypassing tunnel:"
	list_outside

	reroute

	exit $exit_code

# Move process INSIDE tunnel
elif [ "$action" = "inside" ]; then
	for process in "$@"
	do
	    if [ "$process" -eq "$process" ] 2>/dev/null; then
			# Is integer (PID)
			echo "$process" | sudo tee "$cgroup_base/cgroup.procs" > /dev/null
		else
			# Is process name
			pids=$(pidof "$process")
			for pid in $pids
			do
				echo "$pid" | sudo tee "$cgroup_base/cgroup.procs" > /dev/null
			done
		fi
	done
	echo "Remaining processes bypassing tunnel:"
	list_outside


# INFO
elif [ "$action" = "info" ]; then
  echo
  echo "Displaying information for cgroup v$cgroup_version"
  echo
	echo -e "\e[2mcat /etc/iproute2/rt_tables | grep --color \"^${ip_table_number}\s\|\\$\"\e[0m"
	cat /etc/iproute2/rt_tables | grep --color "^${ip_table_number}\s\|\$"
  echo
	echo -e "\e[2mip route show table all | grep -v 'table local' | grep --color 'table\|$'\e[0m"
	ip route show table all | grep -v 'table local' | grep --color 'table\|$'
  echo
	echo -e "\e[2mip rule list | grep --color 'fwmark\|$'\e[0m"
	ip rule list | grep --color 'fwmark\|$'
  echo
	echo -e "\e[2msudo iptables -t mangle -L -v --line-numbers | grep --color 'cgroup\|$'\e[0m"
	sudo iptables -t mangle -L -v --line-numbers | grep --color 'cgroup\|$'
  echo
	echo -e "\e[2msudo iptables -t nat -L -v --line-numbers | grep --color 'cgroup\|$'\e[0m"
	sudo iptables -t nat -L -v --line-numbers | grep --color 'cgroup\|$'
  echo
	echo -e "\e[2mls -l ${cgroup_base}/ | grep --color \"${cgroup_name}\|\\$\"\e[0m"
	ls -l ${cgroup_base}/ | grep --color "${cgroup_name}\|\$"
  echo
	echo -e "\e[2mls -l ${cgroup_base}/${cgroup_name}/ | grep --color \"${USER}\|\\$\"\e[0m"
	ls -l ${cgroup_base}/${cgroup_name}/ | grep --color "${USER}\|\$"
  echo
	echo -e "End of debug information."


# CLEAN the mess
elif [ "$action" = "clean" ]; then
	echo -e "Cleaning forced routing config generated by this script for cgroup version $cgroup_version."
  echo -e "Note: Cleaning partially cleans/breaks the custom configuration for the other cgroup version."
  echo -e "      You should also clean the other cgroup version using the -2 (v2) or no (v1) flag."
  echo -e "Note: If you used a custom cgroup name through --group, you must use it with --clean."
  echo -e "Note: Use --info and --info -2 afterwards to check."
	echo -e "Don't bother with errors meaning there's nothing to remove."

	# Remove tasks
	if [ -f "$cgroup_base/${cgroup_name}/cgroup.procs" ]; then
  	cat "$cgroup_base/${cgroup_name}/cgroup.procs" | \
    while read task_pid
  		do
  			echo ${task_pid} | sudo tee "$cgroup_base/cgroup.procs" > /dev/null
  	done
	fi

	# Delete cgroup
  if [ -d "$cgroup_base/${cgroup_name}" ]; then
    echo -n "Are you sure you want to delete cgroup $cgroup_base/${cgroup_name} ? [y/n] "
    old_stty_cfg=$(stty -g)
    stty raw -echo ; answer=$(head -c 1) ; stty $old_stty_cfg
    echo
    if echo "$answer" | grep -iq "^y" ;then
    	sudo find "$cgroup_base/${cgroup_name}" -depth -type d -print -exec rmdir {} \;
    fi
  fi

	# This can cause issues if reverse path filtering is normally disabled on the system
	echo 1 | sudo tee "/proc/sys/net/ipv4/conf/all/rp_filter" > /dev/null
	echo 1 | sudo tee "/proc/sys/net/ipv4/conf/${real_interface}/rp_filter" > /dev/null

	sudo iptables -t mangle -D OUTPUT -m cgroup $iptables_arg -j MARK --set-mark "$ip_table_fwmark"
	sudo iptables -t nat -D POSTROUTING -m cgroup $iptables_arg -o "$real_interface" -j MASQUERADE

	sudo ip rule del fwmark "$ip_table_fwmark" table "$ip_table_name"
	sudo ip route del default table "$ip_table_name"

	sudo sed -i "/^${ip_table_number}\s/d" /etc/iproute2/rt_tables

  echo "Disabling IPv6 (still - too risky if not managed by VPN)"
  # sudo ip -6 route del blackhole default metric 1
	echo 1 | sudo tee "/proc/sys/net/ipv6/conf/all/disable_ipv6" > /dev/null

	# Redundant
  #if [ -n "`lscgroup net_cls:$cgroup_name`" ]; then
	#	sudo cgdelete net_cls:"$cgroup_name"
	#fi

	echo "All done."

fi

# BONUS: Useful commands:
# ./novpn.sh traceroute www.google.com
# Note: 1 firefox profile = 1 process only
# killall firefox; ./novpn.sh --background firefox https://ipleak.net/
# ip=$(./novpn.sh curl 'https://wtfismyip.com/text' 2>/dev/null); echo "$ip"; whois "$ip" | grep -E "inetnum|route|netname|descr"

