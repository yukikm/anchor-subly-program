#!/bin/bash

# SUBLY Protocol Deployment Script
# This script deploys the SUBLY program and initializes the protocol

set -e

echo "🚀 Starting SUBLY Protocol Deployment..."

# Check if Anchor is installed
if ! command -v anchor &> /dev/null; then
    echo "❌ Anchor CLI is not installed. Please install it first."
    exit 1
fi

# Check if Solana CLI is installed
if ! command -v solana &> /dev/null; then
    echo "❌ Solana CLI is not installed. Please install it first."
    exit 1
fi

# Configuration
CLUSTER=${1:-"devnet"}  # Default to devnet, can pass "mainnet-beta" or "testnet"
AUTHORITY_KEYPAIR=${2:-"~/.config/solana/id.json"}

echo "📋 Deployment Configuration:"
echo "   Cluster: $CLUSTER"
echo "   Authority: $AUTHORITY_KEYPAIR"
echo ""

# Set cluster
echo "🔧 Setting cluster to $CLUSTER..."
solana config set --url $CLUSTER

# Check authority balance
echo "💰 Checking authority balance..."
BALANCE=$(solana balance --keypair $AUTHORITY_KEYPAIR | cut -d' ' -f1)
echo "   Authority balance: $BALANCE SOL"

if (( $(echo "$BALANCE < 5" | bc -l) )); then
    echo "⚠️  Warning: Authority balance is low. Consider adding more SOL for deployment."
    if [ "$CLUSTER" != "mainnet-beta" ]; then
        echo "🪂 Requesting airdrop for devnet/testnet..."
        solana airdrop 5 --keypair $AUTHORITY_KEYPAIR || true
    fi
fi

# Build the program
echo "🔨 Building the program..."
anchor build

# Get program ID
PROGRAM_ID=$(anchor keys list | grep "subly_program" | cut -d':' -f2 | tr -d ' ')
echo "📝 Program ID: $PROGRAM_ID"

# Deploy the program
echo "🚀 Deploying the program..."
anchor deploy --provider.cluster $CLUSTER

# Verify deployment
echo "✅ Verifying deployment..."
solana program show $PROGRAM_ID

echo ""
echo "🎉 SUBLY Protocol deployed successfully!"
echo ""
echo "📋 Deployment Summary:"
echo "   Program ID: $PROGRAM_ID"
echo "   Cluster: $CLUSTER"
echo "   Authority: $(solana-keygen pubkey $AUTHORITY_KEYPAIR)"
echo ""
echo "🔗 Next Steps:"
echo "1. Initialize the protocol using the initialize instruction"
echo "2. Register providers and create subscription services"
echo "3. Configure frontend applications with the new Program ID"
echo ""
echo "📚 For more information, see SUBLY_ARCHITECTURE.md"
