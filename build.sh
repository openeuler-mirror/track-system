#!/bin/bash

	echo "rust not installed, install for current user with rustup.rs ..."
	
	export RUSTUP_DIST_SERVER="https://rsproxy.cn"
	export RUSTUP_UPDATE_ROOT="https://rsproxy.cn/rustup"	
	curl --proto '=https' --tlsv1.2 https://sh.rustup.rs >> .xrustup.sh
	sh .xrustup.sh -y

	if [ $? -ne  0 ]; then
		echo "fail to install rust !!!"
		exit 127
	fi
	mkdir -p $HOME/.cargo
cat > $HOME/.cargo/config.toml <<EOF
[source.crates-io]
