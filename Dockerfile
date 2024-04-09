FROM rust:slim

# based on https://github.com/esp-rs/xtensa-toolchain/blob/main/action.yaml

# for ci scripts run
RUN <<EOF 
apt update
apt install -y \
	curl \
	binutils-xtensa-lx106 \
	gcc-xtensa-lx106 \
	unzip \
	nodejs \
	python3 \
	python3-pip \
	python3-virtualenv \
	libxml2 \
	git
apt clean
rm -rf /var/lib/apt/lists/*
EOF

# install ldproxy
RUN <<EOF
mkdir -p $HOME/.cargo/bin
curl -L https://github.com/esp-rs/embuild/releases/latest/download/ldproxy-x86_64-unknown-linux-gnu.zip -o $HOME/ldproxy.zip
unzip $HOME/ldproxy.zip -d "$HOME/.cargo/bin" 
chmod a+x "$HOME/.cargo/bin/ldproxy"
EOF

# Install espup
RUN <<EOF
curl -L https://github.com/esp-rs/espup/releases/latest/download/espup-x86_64-unknown-linux-gnu -o "$HOME/.cargo/bin/espup"
chmod a+x "$HOME/.cargo/bin/espup"
EOF

# Install esp32 toolchain
RUN <<EOF
"$HOME/.cargo/bin/espup" install -l debug --export-file $HOME/exports --targets esp32
source "$HOME/exports"
rustup default esp
ln -s $(grep "export PATH" "$HOME/exports" | cut -d = -f 2 | sed s/\"//g | cut -d : -f 1) /usr/local/bin/esp-gcc
EOF

ENV PATH=/usr/local/bin/esp-gcc:/root/.cargo/bin:$PATH
