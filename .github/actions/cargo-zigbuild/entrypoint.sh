#!/bin/bash

TARGET="$1"

shift

cargo zigbuild "--target=${TARGET}" $@