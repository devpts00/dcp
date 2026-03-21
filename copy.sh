#!/usr/bin/env bash
sudo sync; echo 1 | sudo tee /proc/sys/vm/drop_caches
time cp -v --reflink=never ./dat/donna.mkv ./dat/donna.copy.mkv