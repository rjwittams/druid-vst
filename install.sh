#!/bin/bash
./osx_vst_bundler.sh Druid2  target/debug/libdruid_vst.dylib
rm -rf ~/Library/Audio/Plug-Ins/VST/Druid2.vst 
mv Druid2.vst ~/Library/Audio/Plug-Ins/VST/Druid2.vst
