#!/bin/bash
./osx_vst_bundler.sh DruidLadderFilter  target/debug/libladder_filter_vst.dylib
rm -rf ~/Library/Audio/Plug-Ins/VST/DruidLadderFilter.vst
mv DruidLadderFilter.vst ~/Library/Audio/Plug-Ins/VST/DruidLadderFilter.vst
