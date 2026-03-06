#!/bin/bash
# Build Handy with CUDA support if CUDA Toolkit is available, otherwise Vulkan-only.

if [ -n "$CUDA_PATH" ] || command -v nvcc &>/dev/null; then
  echo "CUDA Toolkit detected, building with CUDA support..."
  bun run tauri build -- --features cuda
else
  echo "CUDA Toolkit not found, building with Vulkan only..."
  bun run tauri build
fi
