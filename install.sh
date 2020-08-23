# 
# INSTALL LOPEZ
#
# This script install `lopez` in your compute globally for all users (requires
# sudo powers), building the executable from source.
#

echo "Building Lopez from source"
cargo build --release --all &&
echo 'Installing `lopez` to `/usr/local/bin`
Will need `sudo` for this...' &&
sudo cp target/release/lopez /usr/local/bin &&
echo 'Installing `std-lopez` to `/usr/share/lopez/`' &&
sudo mkdir -p /usr/share/lopez/lib &&
sudo cp -r std-lopez/* /usr/share/lopez/lib &&
echo 'Erfolgreich!'
