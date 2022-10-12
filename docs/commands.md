---
title: "Enclaver Commands"
layout: docs-enclaver-single
category: reference
weight: 10
---

# Enclaver Commands

Enclaver is shipped as a single binary that fulfills two main use-cases:

1. Build enclave images, sign them and calculate attestations locally on a developer's machine
1. Bootstrap and run the Nitro enclave on an EC2 machine

Enclaver accepts configuration from command line flags, environment variables, and from a configuration file for builds. When all three are present, the order of precedence is `flag > env var > config file`.

All environment variables are prefixed with `ENC_` and exclusively use underscores. Flags exclusively use dashes. Configuration file parameters exclusively use underscores. For example, `--cpu-count` flag and `ENC_CPU_COUNT` configure the same behavior. 

When overriding a configuration file parameter that is nested, like `image > from`, flatten it like so: `--image-from` or `ENC_IMAGE_FROM`.

## Build

```sh
$ enclaver build [options]
```

Builds an OCI container image in [Enclaver image format][format] containing the components that [run outside][outside] and [inside the enclave][inside]. Once built, the container is tagged with the `name` and `output_tag` field of your enclave configuration file.

| Flag | Type | Description |
|:-----|:-----|:------------|
| `-f`, `--file` | String (Default=enclaver.yaml) | Path on disk to your enclave configuration file. |
| `--eif-only` | String | If set, build only the components that run inside of the enclave. EIF is written to the provided path on disk and the containing directory must exist. |

## Run

```sh
$ enclaver run
```

| Flag | Type | Description |
|:-----|:-----|:------------|
| TODO | TODO | TODO: add options |

## Run-EIF

```sh
$ enclaver run-eif [file] [cpus] [memory] [debug]
```

Runs the given EIF file as an enclave without starting the outside components. Useful to debug enclave startup without needing all of the other components running.

| Flag | Type | Description |
|:-----|:-----|:------------|
| `--eif-file` | String | Path on disk to EIF file to run. |
| `--manifest-file` | String | Path on disk to the manifest file used to generate the EIF. |
| `--cpu-count` | Int | Number of CPUs dedicated to the enclave. |
| `--memory-mb` | Int | Megabytes of memory dedicated to the enclave. |
| `--debug-mode` | Boolean (Default=false) | Enable debug mode on the enclave, which grants access to streaming logs from within. |


[format]: architecture.md#enclaver-image-format
[outside]: architecture.md#components-outside-the-enclave
[inside]: architecture.md#components-inside-the-enclave
