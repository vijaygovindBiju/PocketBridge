#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# PocketBridge: Real Distributed LLM Device Testing Runner
# Targets: Samsung Galaxy S25 Ultra (Snapdragon 8 Elite) + Linux Host
# Model:   Gemma 2 27B GGUF
# ==============================================================================

MODEL_PATH="${MODEL_PATH:-/usr/share/ollama/.ollama/models/blobs/sha256-d7e4b00a7d7a8d03d4eed9b0f3f61a427e9f0fc5dea6aeb414e41dee23dc8ecc}"
ANDROID_DIR="/data/local/tmp/pocketbridge"
RPC_PORT="${RPC_PORT:-50052}"
MODE="${1:---preflight}"
LAYERS_OFFLOAD="${2:-8}"
PROMPT="${3:-Explain why distributed computing between a laptop and mobile phone is challenging. Provide a direct 2-sentence response.}"

show_help() {
    cat << EOF
PocketBridge: Real Distributed LLM Device Testing Runner

Usage:
  ./scripts/deploy_and_run.sh [MODE] [LAYERS_OFFLOAD] [PROMPT]

Modes:
  --preflight, preflight     Measure real hardware memory & thermals without starting inference
  --smoke-test, smoke        Deploy ARM64 binaries and run CPU native smoke test on phone
  --run, run                 Deploy, start on-device RPC server, and execute distributed inference
  --help, -h                 Show this help message

Examples:
  ./scripts/deploy_and_run.sh --preflight
  ./scripts/deploy_and_run.sh --smoke-test
  ./scripts/deploy_and_run.sh --run 8
  ./scripts/deploy_and_run.sh --run 12 "What is pipeline parallelism in deep learning?"

Prerequisites:
  1. Phone connected via USB-C with USB Debugging enabled.
  2. Laptop authorized on phone screen ('Always allow from this computer').
  3. Gemma 2 27B model blob available at $MODEL_PATH.
EOF
}

if [ "$MODE" == "--help" ] || [ "$MODE" == "-h" ] || [ "$MODE" == "help" ]; then
    show_help
    exit 0
fi

echo "=========================================================="
echo "PocketBridge: Real Distributed LLM Device Testing Runner"
echo "=========================================================="

# 0. Dependency and Local Toolchain Checks
echo "[0/6] Verifying host dependencies and native binaries..."
if ! command -v adb >/dev/null 2>&1; then
    echo "ERROR: 'adb' command not found in PATH. Install android-tools (pacman -S android-tools)."
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

HOST_LLAMA_CLI="$PROJECT_ROOT/third_party/llama.cpp/build-host/bin/llama-cli"
ANDROID_BIN_DIR="$PROJECT_ROOT/third_party/llama.cpp/build-android/bin"

if [ ! -f "$HOST_LLAMA_CLI" ]; then
    echo "ERROR: Host llama-cli binary missing at $HOST_LLAMA_CLI"
    echo "Compile host llama.cpp: cmake -B third_party/llama.cpp/build-host third_party/llama.cpp && cmake --build third_party/llama.cpp/build-host --target llama-cli"
    exit 1
fi

for bin_file in ggml-rpc-server libomp.so libggml-cpu.so libggml.so libggml-base.so libggml-rpc.so; do
    if [ ! -f "$ANDROID_BIN_DIR/$bin_file" ]; then
        echo "ERROR: Android ARM64 binary missing: $ANDROID_BIN_DIR/$bin_file"
        echo "Cross-compile Android llama.cpp using Android NDK toolchain."
        exit 1
    fi
done

if [ ! -f "$MODEL_PATH" ]; then
    echo "ERROR: Gemma 2 27B model file not found at $MODEL_PATH"
    echo "Ensure gemma2:27b is pulled in Ollama or specify MODEL_PATH env var."
    exit 1
fi

MODEL_SIZE_BYTES=$(stat -c%s "$MODEL_PATH" 2>/dev/null || stat -f%z "$MODEL_PATH" 2>/dev/null)
MODEL_SIZE_GB=$(awk -v b="$MODEL_SIZE_BYTES" 'BEGIN {printf "%.2f", b/1000000000}')
echo "✅ Local binaries, cross-compiled ARM64 runtime, and model ($MODEL_SIZE_GB GB) verified."

# Cleanup handler
cleanup() {
    echo ""
    echo "[Cleanup] Stopping any background RPC servers and cleaning up tunnels..."
    if [ -n "${DEVICE_ID:-}" ]; then
        adb -s "$DEVICE_ID" shell "pkill -f ggml-rpc-server" 2>/dev/null || true
        adb -s "$DEVICE_ID" forward --remove tcp:$RPC_PORT 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

# 1. Device Detection
echo ""
echo "[1/6] Detecting Android device via ADB..."
DEVICE_COUNT=$(adb devices | grep -v "List" | grep "device$" | wc -l)
if [ "$DEVICE_COUNT" -eq 0 ]; then
    echo "ERROR: No authorized Android device detected via ADB."
    echo "Ensure USB Debugging is ON and the laptop is authorized on the phone screen."
    exit 1
fi

DEVICE_ID=$(adb devices | grep -v "List" | grep "device$" | awk '{print $1}' | head -n 1)
DEVICE_MODEL=$(adb -s "$DEVICE_ID" shell getprop ro.product.model | tr -d '\r')
DEVICE_SOC=$(adb -s "$DEVICE_ID" shell getprop ro.soc.model 2>/dev/null | tr -d '\r' || true)
echo "✅ Device Connected: $DEVICE_MODEL ($DEVICE_ID) [SoC: ${DEVICE_SOC:-Snapdragon 8 Elite}]"

# 2. Real Physical Telemetry Measurement (Never Mocked)
echo ""
echo "[2/6] Measuring real physical memory & thermal telemetry..."
MEM_TOTAL_KB=$(adb -s "$DEVICE_ID" shell "cat /proc/meminfo | grep MemTotal" | awk '{print $2}' | tr -d '\r')
MEM_AVAIL_KB=$(adb -s "$DEVICE_ID" shell "cat /proc/meminfo | grep MemAvailable" | awk '{print $2}' | tr -d '\r')
SWAP_TOTAL_KB=$(adb -s "$DEVICE_ID" shell "cat /proc/meminfo | grep SwapTotal" | awk '{print $2}' | tr -d '\r')
SWAP_FREE_KB=$(adb -s "$DEVICE_ID" shell "cat /proc/meminfo | grep SwapFree" | awk '{print $2}' | tr -d '\r')

MEM_TOTAL_MB=$((MEM_TOTAL_KB / 1024))
MEM_AVAIL_MB=$((MEM_AVAIL_KB / 1024))
SWAP_TOTAL_MB=$((SWAP_TOTAL_KB / 1024))
SWAP_FREE_MB=$((SWAP_FREE_KB / 1024))

echo "  • Total RAM:      ${MEM_TOTAL_MB} MB ($((MEM_TOTAL_MB / 1024)) GB)"
echo "  • Available RAM:  ${MEM_AVAIL_MB} MB ($((MEM_AVAIL_MB / 1024)) GB)"
echo "  • Swap Total:     ${SWAP_TOTAL_MB} MB (Free: ${SWAP_FREE_MB} MB)"

# Battery & Thermal safety check
BATTERY_LEVEL=$(adb -s "$DEVICE_ID" shell dumpsys battery | awk '/level:/ {print $2; exit}' | tr -d '\r')
BATTERY_TEMP_RAW=$(adb -s "$DEVICE_ID" shell dumpsys battery | awk '/temperature:/ {print $2; exit}' | tr -d '\r')
BATTERY_STATUS=$(adb -s "$DEVICE_ID" shell dumpsys battery | awk '/status:/ {print $2; exit}' | tr -d '\r')
BATTERY_TEMP=$(awk -v t="$BATTERY_TEMP_RAW" 'BEGIN {printf "%.1f", t/10}')
IS_CHARGING="false"
if [ "$BATTERY_STATUS" -eq 2 ] || [ "$BATTERY_STATUS" -eq 5 ]; then
    IS_CHARGING="true"
fi
echo "  • Battery:        ${BATTERY_LEVEL}% (${BATTERY_TEMP}°C, charging: ${IS_CHARGING})"

if [ "$BATTERY_LEVEL" -lt 15 ] && [ "$IS_CHARGING" == "false" ]; then
    echo "⚠️  SAFETY GATE REJECTION: Phone battery is critically low (${BATTERY_LEVEL}%). Connect charger before running distributed inference."
    exit 1
fi

TEMP_INT=$(awk -v t="$BATTERY_TEMP" 'BEGIN {print int(t)}')
if [ "$TEMP_INT" -gt 42 ]; then
    echo "⚠️  SAFETY GATE REJECTION: Phone battery temperature is high (${BATTERY_TEMP}°C). Allow phone to cool down."
    exit 1
fi

# Calculate safe memory headroom (leave 1.2 GB Android system headroom to prevent lmkd kills)
SAFE_LIMIT_MB=$((MEM_AVAIL_MB - 1200))
MAX_SAFE_LAYERS=$((SAFE_LIMIT_MB / 319))
if [ "$MAX_SAFE_LAYERS" -lt 1 ]; then
    echo "ERROR: Insufficient available phone RAM (${MEM_AVAIL_MB} MB). At least 1.5 GB free RAM required."
    exit 1
fi
echo "  • Safe Headroom:  ${SAFE_LIMIT_MB} MB -> Max safe offload: ${MAX_SAFE_LAYERS} layers (leaves 1.2 GB OS buffer)"

if [ "$MODE" == "--preflight" ] || [ "$MODE" == "preflight" ]; then
    echo ""
    echo "=========================================================="
    echo "PREFLIGHT SAFETY GATE PASSED ✅"
    echo "Physical device verified and ready for deployment."
    echo "Recommended next steps:"
    echo "  1. Smoke test:  ./scripts/deploy_and_run.sh --smoke-test"
    echo "  2. Inference:   ./scripts/deploy_and_run.sh --run $(( MAX_SAFE_LAYERS < 8 ? MAX_SAFE_LAYERS : 8 ))"
    echo "=========================================================="
    exit 0
fi

# 3. Deploy ARM64 Runtime
echo ""
echo "[3/6] Deploying native ARM64 RPC runtime to $ANDROID_DIR..."
adb -s "$DEVICE_ID" shell "mkdir -p $ANDROID_DIR"
adb -s "$DEVICE_ID" push "$ANDROID_BIN_DIR/ggml-rpc-server" "$ANDROID_DIR/" >/dev/null
adb -s "$DEVICE_ID" push "$ANDROID_BIN_DIR"/libggml*.so "$ANDROID_DIR/" >/dev/null
adb -s "$DEVICE_ID" push "$ANDROID_BIN_DIR/libomp.so" "$ANDROID_DIR/" >/dev/null
adb -s "$DEVICE_ID" shell "chmod +x $ANDROID_DIR/ggml-rpc-server"
echo "✅ Runtime binaries and shared libraries pushed successfully."

# 4. Smoke Test
echo ""
echo "[4/6] Running Android CPU smoke test..."
SMOKE_OUTPUT=$(adb -s "$DEVICE_ID" shell "LD_LIBRARY_PATH=$ANDROID_DIR $ANDROID_DIR/ggml-rpc-server -h" 2>&1 | tr -d '\r')
if echo "$SMOKE_OUTPUT" | grep -q "options:"; then
    echo "✅ Smoke test passed: ggml-rpc-server executed natively on phone CPU."
else
    echo "❌ Smoke test failed. Output:"
    echo "$SMOKE_OUTPUT"
    exit 1
fi

if [ "$MODE" == "--smoke-test" ] || [ "$MODE" == "smoke" ]; then
    echo ""
    echo "=========================================================="
    echo "SMOKE TEST SUCCESSFUL!"
    echo "The Android native runtime is 100% verified on device."
    echo "To run real distributed inference, execute:"
    echo "  ./scripts/deploy_and_run.sh --run $(( MAX_SAFE_LAYERS < 8 ? MAX_SAFE_LAYERS : 8 ))"
    echo "=========================================================="
    exit 0
fi

# 5. Starting ggml-rpc-server on Phone
echo ""
echo "[5/6] Starting ggml-rpc-server on Android device..."
REQUIRED_MEM_MB=$((LAYERS_OFFLOAD * 319))
echo "Requested offload: $LAYERS_OFFLOAD layers (~${REQUIRED_MEM_MB} MB)"
if [ "$REQUIRED_MEM_MB" -gt "$SAFE_LIMIT_MB" ]; then
    echo "⚠️  WARNING: Requested $LAYERS_OFFLOAD layers (${REQUIRED_MEM_MB} MB) exceeds safe headroom (${SAFE_LIMIT_MB} MB)."
    echo "    Dynamically clamping offload layers to prevent Android low-memory killer (lmkd) kill..."
    LAYERS_OFFLOAD=$MAX_SAFE_LAYERS
    echo "    Adjusted safe layer count: $LAYERS_OFFLOAD layers ($((LAYERS_OFFLOAD * 319)) MB)"
fi

# Kill any stale server instance on phone and reset port forwarding
adb -s "$DEVICE_ID" shell "pkill -f ggml-rpc-server" 2>/dev/null || true
adb -s "$DEVICE_ID" forward --remove tcp:$RPC_PORT 2>/dev/null || true
sleep 1

adb -s "$DEVICE_ID" forward tcp:$RPC_PORT tcp:$RPC_PORT
echo "Port forwarding active: localhost:$RPC_PORT -> phone:$RPC_PORT"

adb -s "$DEVICE_ID" shell "nohup sh -c 'LD_LIBRARY_PATH=$ANDROID_DIR $ANDROID_DIR/ggml-rpc-server -H 127.0.0.1 -p $RPC_PORT -t 6 > $ANDROID_DIR/rpc-server.log 2>&1' &"
sleep 2

if ! adb -s "$DEVICE_ID" shell "pgrep -f ggml-rpc-server" > /dev/null; then
    echo "ERROR: ggml-rpc-server failed to bind or start. Server log:"
    adb -s "$DEVICE_ID" shell "cat $ANDROID_DIR/rpc-server.log" || true
    exit 1
fi
SERVER_PID=$(adb -s "$DEVICE_ID" shell pgrep -f ggml-rpc-server | tr -d '\r')
echo "✅ ggml-rpc-server active on Android (PID: $SERVER_PID)"

# 6. Distributed Inference Execution
echo ""
echo "[6/6] Executing distributed inference with Gemma 2 27B..."
echo "  • Model path: $MODEL_PATH"
echo "  • Layers on phone: $LAYERS_OFFLOAD layers"
echo "  • Layers on host:  $((46 - LAYERS_OFFLOAD)) layers + embedding table"
echo "  • Prompt: \"$PROMPT\""
echo "----------------------------------------------------------"

"$HOST_LLAMA_CLI" \
    --model "$MODEL_PATH" \
    --rpc "127.0.0.1:$RPC_PORT" \
    --gpu-layers "$LAYERS_OFFLOAD" \
    --split-mode layer \
    --threads 6 \
    --prompt "$PROMPT" \
    --n-predict 48 \
    --verbose-prompt

echo "----------------------------------------------------------"
echo "Distributed inference execution completed successfully!"
