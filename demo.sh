#!/bin/bash
# Demo script for cargo-distbuild
# This script demonstrates the basic workflow of the distributed build system

set -e

echo "🚀 cargo-distbuild Demo Script"
echo "================================"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if cargo-distbuild is built
if [ ! -f "target/debug/cargo-distbuild" ] && [ ! -f "target/release/cargo-distbuild" ]; then
    echo -e "${YELLOW}Building cargo-distbuild...${NC}"
    cargo build
    echo ""
fi

# Determine binary path
if [ -f "target/release/cargo-distbuild" ]; then
    BIN="target/release/cargo-distbuild"
else
    BIN="target/debug/cargo-distbuild"
fi

echo -e "${GREEN}Using binary: $BIN${NC}"
echo ""

# Create test data
echo -e "${YELLOW}Step 1: Creating test data...${NC}"
echo "Hello from cargo-distbuild demo!" > /tmp/test-input.txt
echo -e "${GREEN}✓ Created /tmp/test-input.txt${NC}"
echo ""

# Put file in CAS
echo -e "${YELLOW}Step 2: Storing file in CAS...${NC}"
OUTPUT=$($BIN cas put /tmp/test-input.txt)
echo "$OUTPUT"
HASH=$(echo "$OUTPUT" | grep "Hash:" | awk '{print $2}')
echo -e "${GREEN}✓ Input hash: $HASH${NC}"
echo ""

# Check scheduler status
echo -e "${YELLOW}Step 3: Checking scheduler status...${NC}"
$BIN scheduler status || echo -e "${RED}Scheduler not running. Start it with: cargo run -- scheduler run${NC}"
echo ""

# List workers
echo -e "${YELLOW}Step 4: Listing registered workers...${NC}"
$BIN master list-workers || echo -e "${RED}Failed to connect to scheduler${NC}"
echo ""

# Submit a job (if scheduler is running)
echo -e "${YELLOW}Step 5: Submitting a job...${NC}"
if $BIN scheduler status > /dev/null 2>&1; then
    JOB_OUTPUT=$($BIN master submit-job "$HASH" 2>&1 || echo "failed")
    if [[ "$JOB_OUTPUT" != "failed" ]]; then
        echo "$JOB_OUTPUT"
        JOB_ID=$(echo "$JOB_OUTPUT" | grep "Job ID:" | awk '{print $3}')
        echo -e "${GREEN}✓ Job submitted: $JOB_ID${NC}"
        echo ""
        
        # Wait a bit
        echo -e "${YELLOW}Waiting 2 seconds for job processing...${NC}"
        sleep 2
        echo ""
        
        # Check job status
        echo -e "${YELLOW}Step 6: Checking job status...${NC}"
        $BIN master job-status "$JOB_ID"
        echo ""
    else
        echo -e "${RED}Failed to submit job${NC}"
    fi
else
    echo -e "${RED}Scheduler not running. Skipping job submission.${NC}"
    echo -e "${YELLOW}To run the full demo:${NC}"
    echo "  1. Terminal 1: cargo run -- scheduler run"
    echo "  2. Terminal 2: cargo run -- worker run --id worker-1 --port 6001"
    echo "  3. Terminal 3: ./demo.sh"
fi
echo ""

# List all blobs in CAS
echo -e "${YELLOW}Step 7: Listing all blobs in CAS...${NC}"
$BIN cas list
echo ""

# Interactive mode info
echo -e "${GREEN}═══════════════════════════════════════${NC}"
echo -e "${GREEN}Demo completed!${NC}"
echo ""
echo "To explore interactively, run:"
echo -e "  ${YELLOW}cargo run${NC}"
echo ""
echo "Then try these commands:"
echo "  cas put <file>"
echo "  job submit <hash>"
echo "  jobs list"
echo "  workers list"
echo "  help"
echo ""
echo -e "${GREEN}═══════════════════════════════════════${NC}"

