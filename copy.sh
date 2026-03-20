#!/usr/bin/env bash
sudo sync; echo 1 | sudo tee /proc/sys/vm/drop_caches
time cp --reflink=never /run/media/akz/36d01703-71d5-434d-9404-1111fb1e9ec6/yakuza.mkv ./dat/yakuza.mkv