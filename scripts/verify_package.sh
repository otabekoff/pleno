#!/bin/bash
set -e

# Setup temp dir
TEST_DIR="verification_test_pkg"
rm -rf $TEST_DIR
mkdir -p $TEST_DIR
cd $TEST_DIR

# 1. Test init
echo "Testing init..."
../target/debug/pleno-cli init mytestapp
if [ ! -f pleno.toml ]; then
    echo "FAILED: pleno.toml not created"
    exit 1
fi

# 2. Test install
echo "Testing install..."
# Create a valid pleno.toml first if init didn't (cli init creates it though)
../target/debug/pleno-cli install mylib
if [ ! -f packages/mylib/lib.pln ]; then
    echo "FAILED: packages/mylib/lib.pln not created"
    exit 1
fi
echo "export fn hello() { println(\"Hello from mylib\"); }" > packages/mylib/lib.pln

# 3. Create main script using import
echo "Testing import..."
echo 'import mylib from "mylib";' > main.pln
echo 'mylib.hello();' >> main.pln

# 4. Run main script
../target/debug/pleno-cli run main.pln
